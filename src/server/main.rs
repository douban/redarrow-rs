use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll};

use actix_web::{get, middleware, web, App, HttpResponse, HttpServer, Responder};
use argh::FromArgs;
use bytes::Bytes;
use futures::executor;
use futures::Stream;
use tokio::signal::unix::{signal, SignalKind};

use redarrow::dispatcher::{read_config, Configs, RedarrowWaker};
use redarrow::{CommandParams, CommandResult};

#[argh(description = "execute command for remote redarrow client")]
#[derive(FromArgs, Debug)]
struct ServerArgs {
    #[argh(
        option,
        short = 'c',
        default = r#""/etc/redarrow.conf".to_string()"#,
        description = "path to config file"
    )]
    config: String,

    #[argh(
        option,
        short = 'p',
        default = "4205",
        description = "redarrow service port"
    )]
    port: u16,

    #[argh(
        option,
        short = 'w',
        default = "4",
        description = "number of worker processes for handling requests"
    )]
    workers: usize,
}

#[actix_rt::main]
async fn main() -> std::io::Result<()> {
    env_logger::init();
    let args: ServerArgs = argh::from_env();

    let configs = match read_config(args.config.as_str()) {
        Ok(c) => {
            log::info!("parsed {} commands, starting server...", &c.len());
            c
        }
        Err(e) => {
            log::error!("parse config error: {}", e);
            return Ok(());
        }
    };

    let (tx, rx) = std::sync::mpsc::channel::<&str>();

    let server = HttpServer::new(move || {
        App::new()
            .data(configs.clone())
            .wrap(middleware::Logger::default())
            .service(handlers_command)
    })
    .bind(format!("0.0.0.0:{}", args.port))?
    .workers(args.workers)
    .run();

    let srv = server.clone();
    std::thread::Builder::new()
        .name("signal receiver".to_string())
        .spawn(move || loop {
            let sig = rx.recv().unwrap_or("");
            match sig {
                "TERM" => {
                    // stop server gracefully
                    executor::block_on(srv.stop(true));
                    break;
                }
                "HUP" => {}
                _ => {
                    // wait 10ms if recv error
                    std::thread::sleep(std::time::Duration::from_millis(10));
                }
            }
        })
        .unwrap();

    let mut stream_hup = signal(SignalKind::hangup())?;
    let hup_tx = tx.clone();
    actix_rt::spawn(async move {
        loop {
            stream_hup.recv().await;
            // TODO:(everpcpc) impl reload
            log::info!("SIGHUP received. Reloading...");
            hup_tx.send("TERM").unwrap();
        }
    });
    let mut stream_term = signal(SignalKind::terminate())?;
    let term_tx = tx.clone();
    actix_rt::spawn(async move {
        loop {
            stream_term.recv().await;
            log::info!("SIGTERM received. Terminating...");
            term_tx.send("TERM").unwrap();
        }
    });

    server.await
}

#[get("command/{command}")]
async fn handlers_command(
    command: web::Path<String>,
    opts: web::Query<CommandParams>,
    configs: web::Data<Configs>,
) -> impl Responder {
    let chunked = match opts.chunked {
        None => false,
        Some(c) => c != 0,
    };
    let arguments = match &opts.argument {
        None => Vec::new(),
        Some(a) => a.split(" ").map(|x| x.to_string()).collect(),
    };

    if chunked {
        handle_command_chunked(command.as_str(), arguments, configs)
    } else {
        handle_command_no_chunked(command.as_str(), arguments, configs)
    }
}

fn handle_command_chunked(
    command: &str,
    arguments: Vec<String>,
    configs: web::Data<Configs>,
) -> HttpResponse {
    match configs.get(command) {
        None => HttpResponse::BadRequest().body(format!(
            "0> {}\n",
            CommandResult::err(format!("Unknown Command: {}", command)).to_json()
        )),
        Some(cmd) => {
            let (tx_cmd, rx_cmd) = std::sync::mpsc::channel::<String>();
            let waker = Arc::new(Mutex::new(RedarrowWaker::new()));
            let cmd = cmd.clone();
            let mut wake_sender = waker.clone();
            match std::thread::Builder::new()
                .name(format!("runner for {}", command))
                .spawn(move || {
                    let ret = format!(
                        "0> {}\n",
                        cmd.execute_iter(arguments, tx_cmd.clone(), &mut wake_sender)
                            .unwrap_or_else(|err| CommandResult::err(format!("{}", err)))
                            .to_json()
                    );
                    match tx_cmd.send(ret) {
                        Err(e) => {
                            log::warn!("send command result error: {}", e);
                            return;
                        }
                        Ok(()) => {
                            if let Ok(mut waker) = wake_sender.lock() {
                                waker.wake();
                            } else {
                                log::warn!("waker on command result failed to get lock");
                            }
                        }
                    }
                    // NOTE:(everpcpc) force end recv rx_cmd, do not wait for stdout/stderr
                    match tx_cmd.send("\0".to_string()) {
                        Err(e) => {
                            log::warn!("send command end error: {}", e);
                            return;
                        }
                        Ok(()) => {
                            // NOTE:(everpcpc) acturally this wake always false
                            if let Ok(mut waker) = wake_sender.lock() {
                                waker.wake();
                            } else {
                                log::warn!("waker on command end failed to get lock");
                            }
                        }
                    }
                }) {
                Ok(_) => HttpResponse::Ok().streaming(ChunkedResponse {
                    rx: rx_cmd,
                    waker: waker,
                }),
                Err(e) => HttpResponse::InternalServerError()
                    .json(CommandResult::err(format!("Failed to start task: {}", e))),
            }
        }
    }
}

fn handle_command_no_chunked(
    command: &str,
    arguments: Vec<String>,
    configs: web::Data<Configs>,
) -> HttpResponse {
    match configs.get(command) {
        None => {
            let err = CommandResult::err(format!("Unknown Command: {}", command));
            HttpResponse::BadRequest().json(err)
        }
        Some(cmd) => {
            let r = cmd
                .execute(arguments)
                .unwrap_or_else(|err| CommandResult::err(format!("{}", err)));
            HttpResponse::Ok().json(r)
        }
    }
}

#[derive(Debug)]
struct ChunkedResponse {
    rx: std::sync::mpsc::Receiver<String>,
    waker: Arc<Mutex<RedarrowWaker>>,
}

impl Stream for ChunkedResponse {
    type Item = Result<bytes::Bytes, actix_web::Error>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        match self.rx.try_recv() {
            Err(_) => {
                if let Ok(mut waker) = self.waker.lock() {
                    waker.register(cx.waker());
                } else {
                    log::warn!("register waker failed to get lock");
                }
                Poll::Pending
            }
            Ok(result) => {
                if result == "\0" {
                    Poll::Ready(None)
                } else {
                    Poll::Ready(Some(Ok(Bytes::from(result))))
                }
            }
        }
    }
}

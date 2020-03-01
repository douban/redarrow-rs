use actix_web::{get, middleware, web, App, HttpResponse, HttpServer, Responder};
use argh::FromArgs;
use bytes::Bytes;
use futures::executor;
use serde::Deserialize;
use tokio::signal::unix::{signal, SignalKind};

use redarrow::dispatcher;
use redarrow::result;

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

// The query parameters for command.
#[derive(Debug, Deserialize)]
pub struct CommandOptions {
    argument: Option<String>,
    chunked: Option<u8>,
}

#[actix_rt::main]
async fn main() -> std::io::Result<()> {
    env_logger::init();
    let args: ServerArgs = argh::from_env();

    let configs = match dispatcher::read_config(args.config.as_str()) {
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
    std::thread::spawn(move || loop {
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
    });

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
    opts: web::Query<CommandOptions>,
    configs: web::Data<dispatcher::Configs>,
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
    configs: web::Data<dispatcher::Configs>,
) -> HttpResponse {
    match configs.get(command) {
        None => HttpResponse::BadRequest().body(format!(
            "0> {}\n",
            result::CommandResult::err(format!("Unknown Command: {}", command)).to_json()
        )),
        Some(cmd) => {
            let (tx_body, rx_body) =
                actix_utils::mpsc::channel::<Result<bytes::Bytes, actix_web::Error>>();
            let (tx_cmd, rx_cmd) = std::sync::mpsc::channel::<String>();
            actix_rt::spawn(async move {
                loop {
                    match rx_cmd.recv() {
                        Err(e) => {
                            log::warn!("recv output error: {}", e);
                            break;
                        },
                        Ok(result) => {
                            if result == "\0" {
                                break;
                            }
                            if tx_body.send(Ok(Bytes::from(result))).is_err() {
                                break;
                            };
                            // HACK:(everpcpc) wait 1ns to send
                            futures_timer::Delay::new(std::time::Duration::from_nanos(1)).await;
                        }
                    }
                }
            });
            let cmd = cmd.clone();
            std::thread::spawn(move || {
                let ret = format!(
                    "0> {}\n",
                    cmd.execute_iter(arguments, tx_cmd.clone())
                        .unwrap_or_else(|err| result::CommandResult::err(format!("{}", err)))
                        .to_json()
                );
                if tx_cmd.send(ret).is_err() {
                    return;
                }
                // HACK:(everpcpc) force end recv rx_cmd, do not wait for stdout/stderr
                if tx_cmd.send("\0".to_string()).is_err() {
                    return;
                }
            });
            HttpResponse::Ok().streaming(rx_body)
        }
    }
}

fn handle_command_no_chunked(
    command: &str,
    arguments: Vec<String>,
    configs: web::Data<dispatcher::Configs>,
) -> HttpResponse {
    match configs.get(command) {
        None => {
            let err = result::CommandResult::err(format!("Unknown Command: {}", command));
            HttpResponse::BadRequest().json(err)
        }
        Some(cmd) => {
            let r = cmd
                .execute(arguments)
                .unwrap_or_else(|err| result::CommandResult::err(format!("{}", err)));
            HttpResponse::Ok().json(r)
        }
    }
}

use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll};

use argh::FromArgs;
use futures::Stream;
use tokio::signal::unix::{signal, SignalKind};
use tokio::sync::mpsc;
use warp::http::StatusCode;
use warp::Filter;

use redarrow::dispatcher::{read_config, Command, Configs, RedarrowWaker};
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

#[tokio::main]
async fn main() {
    pretty_env_logger::init_timed();

    let args: ServerArgs = argh::from_env();
    let configs = match read_config(args.config.as_str()) {
        Ok(c) => {
            log::info!("parsed {} commands, starting server...", &c.len());
            c
        }
        Err(e) => {
            log::error!("parse config error: {}", e);
            return;
        }
    };
    let configs = Arc::new(configs);
    let configs = warp::any().map(move || configs.clone());

    let (tx, mut rx) = mpsc::channel::<&str>(2);

    let (addr, server) = warp::serve(
        warp::path("command")
            .and(warp::get())
            .and(warp::path::param::<String>())
            .and(warp::query::<CommandParams>())
            .and(configs)
            .and_then(handlers_command)
            .with(warp::log("redarrow::http")),
    )
    .bind_with_graceful_shutdown(([0, 0, 0, 0], args.port), async move {
        while let Some(res) = rx.recv().await {
            match res {
                "TERM" => break,
                // TODO:(everpcpc) impl reload
                "HUP" => break,
                _ => log::error!("received invalid signal: {}", res),
            }
        }
    });

    log::info!("listening on {}", addr);

    let mut stream_hup = signal(SignalKind::hangup()).unwrap();
    let hup_tx = tx.clone();
    tokio::task::spawn(async move {
        loop {
            stream_hup.recv().await;
            log::info!("SIGHUP received. Reloading...");
            hup_tx.send("HUP").await.unwrap();
        }
    });
    let mut stream_term = signal(SignalKind::terminate()).unwrap();
    let term_tx = tx.clone();
    tokio::task::spawn(async move {
        stream_term.recv().await;
        log::info!("SIGTERM received. Terminating...");
        term_tx.send("TERM").await.unwrap();
    });

    tokio::task::spawn(server).await.unwrap()
}

async fn handlers_command(
    command: String,
    opts: CommandParams,
    configs: Arc<Configs>,
) -> Result<Box<dyn warp::Reply>, std::convert::Infallible> {
    let chunked = match opts.chunked {
        None => false,
        Some(c) => c != 0,
    };
    let arguments = match &opts.argument {
        None => Vec::new(),
        Some(a) => a.split(" ").map(|x| x.to_string()).collect(),
    };
    match configs.get(&command) {
        None => {
            let err = CommandResult::err(format!("Unknown Command: {}", command));
            if chunked {
                Ok(Box::new(warp::reply::with_status(
                    format!("0> {}\n", err.to_json()),
                    StatusCode::BAD_REQUEST,
                )))
            } else {
                Ok(Box::new(warp::reply::with_status(
                    warp::reply::json(&err),
                    StatusCode::BAD_REQUEST,
                )))
            }
        }
        Some(cmd) => {
            if chunked {
                handle_command_chunked(cmd.clone(), arguments)
            } else {
                let ret = match cmd.execute(arguments) {
                    Err(e) => warp::reply::with_status(
                        warp::reply::json(&CommandResult::err(format!("{}", e))),
                        StatusCode::INTERNAL_SERVER_ERROR,
                    ),
                    Ok(r) => warp::reply::with_status(warp::reply::json(&r), StatusCode::OK),
                };
                Ok(Box::new(ret))
            }
        }
    }
}

fn handle_command_chunked(
    cmd: Command,
    arguments: Vec<String>,
) -> Result<Box<dyn warp::Reply>, std::convert::Infallible> {
    let (tx_cmd, rx_cmd) = std::sync::mpsc::channel::<String>();
    let waker = Arc::new(Mutex::new(RedarrowWaker::new()));
    let mut wake_sender = waker.clone();
    let _child = std::thread::spawn(move || {
        let ret = format!(
            "0> {}\n",
            match cmd.execute_iter(arguments, tx_cmd.clone(), &mut wake_sender) {
                Ok(r) => r,
                Err(e) => CommandResult::err(format!("{}", e)),
            }
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
        match tx_cmd.send("\0\0".to_string()) {
            Err(e) => {
                log::warn!("send command end error: {}", e);
                return;
            }
            Ok(()) => {
                if let Ok(mut waker) = wake_sender.lock() {
                    waker.wake();
                } else {
                    log::warn!("waker on command end failed to get lock");
                }
            }
        }
    });
    let r = ChunkedResponse {
        rx: rx_cmd,
        waker: waker,
    };
    let mut res = hyper::Response::new(hyper::Body::empty());
    *res.body_mut() = hyper::Body::wrap_stream(r);
    Ok(Box::new(res))
}

#[derive(Debug)]
struct ChunkedResponse {
    rx: std::sync::mpsc::Receiver<String>,
    waker: Arc<Mutex<RedarrowWaker>>,
}

unsafe impl Sync for ChunkedResponse {}

impl Stream for ChunkedResponse {
    type Item = Result<String, warp::Error>;

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
                if result == "\0\0" {
                    Poll::Ready(None)
                } else {
                    Poll::Ready(Some(Ok(result)))
                }
            }
        }
    }
}

use actix_web::{get, middleware, web, App, HttpResponse, HttpServer, Responder};
use argh::FromArgs;
use bytes::Bytes;
use futures::executor;
use serde::Deserialize;
use tokio::signal::unix::{signal, SignalKind};

use redarrow::dispatcher;

#[argh(description = "execute command for remote redarrow client")]
#[derive(FromArgs, Debug)]
struct ServerArgs {
    #[argh(
        option,
        short = 'c',
        default = "\"/etc/redarrow.conf\".to_string()",
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

    let configs = dispatcher::read_config(args.config.as_str()).unwrap();
    println!("parsed {} commands, starting server...", &configs.len());

    let (tx, rx) = std::sync::mpsc::channel::<&str>();

    let server = HttpServer::new(move || {
        App::new()
            .data(configs.clone())
            .wrap(middleware::Logger::default())
            .service(handlers_command)
    })
    .bind(format!("0.0.0.0:{}", args.port))?
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
            println!("\n*** SIGHUP received. Reloading. ***\n");
            hup_tx.send("TERM").unwrap();
        }
    });
    let mut stream_term = signal(SignalKind::terminate())?;
    let term_tx = tx.clone();
    actix_rt::spawn(async move {
        loop {
            stream_term.recv().await;
            println!("\n*** SIGTERM received. Terminating. ***\n");
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
        Some(a) => a.split(" ").collect(),
    };

    if chunked {
        handle_command_chunked(command.as_str(), arguments, configs)
    } else {
        handle_command_no_chunked(command.as_str(), arguments, configs)
    }
}

fn handle_command_chunked(
    command: &str,
    arguments: Vec<&str>,
    configs: web::Data<dispatcher::Configs>,
) -> HttpResponse {
    match configs.get(command) {
        None => {
            let err = dispatcher::CommandResult::err(format!("Unknown Command: {}", command));
            HttpResponse::BadRequest()
                .body(format!("0> {}\n", serde_json::to_string(&err).unwrap()))
        }
        Some(cmd) => {
            let (tx_body, rx_body) =
                actix_utils::mpsc::channel::<Result<bytes::Bytes, actix_web::Error>>();
            let (tx_cmd, rx_cmd) = std::sync::mpsc::channel::<String>();
            actix_rt::spawn(async move {
                loop {
                    match rx_cmd.recv() {
                        Err(_) => break,
                        Ok(result) => {
                            tx_body.send(Ok(Bytes::from(result))).unwrap();
                            // HACK:(everpcpc) wait 1ns to send
                            futures_timer::Delay::new(std::time::Duration::from_nanos(1)).await;
                        }
                    }
                }
            });
            let cmd = cmd.clone();
            // NOTE:(everpcpc) use Vec<String> to avoid lifetime issue
            let arguments: Vec<String> = arguments.iter().map(|x| x.to_string()).collect();
            std::thread::spawn(move || {
                let r = cmd
                    .execute_iter(
                        arguments.iter().map(|x| x.as_str()).collect(),
                        tx_cmd.clone(),
                    )
                    .unwrap_or_else(|err| dispatcher::CommandResult::err(format!("{}", err)));
                tx_cmd
                    .send(format!("0> {}\n", serde_json::to_string(&r).unwrap()))
                    .unwrap();
            });
            HttpResponse::Ok().streaming(rx_body)
        }
    }
}

fn handle_command_no_chunked(
    command: &str,
    arguments: Vec<&str>,
    configs: web::Data<dispatcher::Configs>,
) -> HttpResponse {
    match configs.get(command) {
        None => {
            let err = dispatcher::CommandResult::err(format!("Unknown Command: {}", command));
            HttpResponse::BadRequest().json(err)
        }
        Some(cmd) => {
            let r = cmd
                .execute(arguments)
                .unwrap_or_else(|err| dispatcher::CommandResult::err(format!("{}", err)));
            HttpResponse::Ok().json(r)
        }
    }
}

// // DEPRECATE:
// fn handle_list_no_chunked(configs: web::Data<dispatcher::Configs>) -> HttpResponse {
//     let r = dispatcher::CommandResult::ok(
//         format!(
//             "Available commands:\n{}\n",
//             configs
//                 .keys()
//                 .map(|x| x.to_string())
//                 .collect::<Vec<String>>()
//                 .join("\n")
//         ),
//         "".to_string(),
//         0,
//         0.0,
//         0.0,
//     );
//     return HttpResponse::Ok().json(r);
// }

// // DEPRECATE:
// fn handle_list_chunked(configs: web::Data<dispatcher::Configs>) -> HttpResponse {
//     let (tx_body, rx_body) = actix_utils::mpsc::channel::<Result<bytes::Bytes, actix_web::Error>>();
//     actix_rt::spawn(async move {
//         tx_body
//             .send(Ok(Bytes::from("1> Available commands:\n")))
//             .unwrap();
//         for key in configs.keys() {
//             tx_body
//                 .send(Ok(Bytes::from(format!("1> {}\n", key))))
//                 .unwrap();
//         }
//         let r = dispatcher::CommandResult::chunked_ok(0, 0.0, 0.0);
//         tx_body
//             .send(Ok(Bytes::from(format!(
//                 "0> {}\n",
//                 serde_json::to_string(&r).unwrap()
//             ))))
//             .unwrap();
//     });
//     return HttpResponse::Ok().streaming(rx_body);
// }

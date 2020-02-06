use actix_web::{get, web, App, HttpResponse, HttpServer, Responder};
use argh::FromArgs;
use bytes::Bytes;
use serde::Deserialize;

use redarrow::dispatcher;

#[argh(description = "execute command for remote redarrow client")]
#[derive(FromArgs, Debug)]
struct ServerArgs {
    #[argh(
        switch,
        short = 'd',
        description = "return text/html instead of application/json"
    )]
    debug: bool,

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
    let args: ServerArgs = argh::from_env();

    let configs = dispatcher::read_config(args.config.as_str()).unwrap();
    println!("config parsed, starting server...");

    HttpServer::new(move || App::new().data(configs.clone()).service(handlers_command))
        .bind(format!("0.0.0.0:{}", args.port))?
        .run()
        .await
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
    // NOTE: use Vec<String> to avoid lifetime issue
    let arguments: Vec<String> = match &opts.argument {
        None => Vec::new(),
        Some(a) => a.split(" ").map(|x| x.to_string()).collect(),
    };

    if !chunked {
        match configs.get(command.as_str()) {
            None => {
                let err = dispatcher::CommandResult::err(format!("Unknown Command: {}", command));
                HttpResponse::BadRequest().json(err)
            }
            Some(cmd) => {
                let r = cmd
                    .execute(arguments.iter().map(|x| x.as_str()).collect())
                    .unwrap_or_else(|err| dispatcher::CommandResult::err(format!("{}", err)));
                HttpResponse::Ok().json(r)
            }
        }
    } else {
        match configs.get(command.as_str()) {
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
                                // HACK: wait 1ns to send
                                futures_timer::Delay::new(std::time::Duration::from_nanos(1)).await;
                            }
                        }
                    }
                });
                let cmd = cmd.clone();
                std::thread::spawn(move || {
                    cmd.execute_iter(arguments.iter().map(|x| x.as_str()).collect(), tx_cmd)
                        .unwrap()
                });
                HttpResponse::Ok().streaming(rx_body)
            }
        }
    }
}

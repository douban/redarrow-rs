use std::convert::Infallible;

use anyhow::Result;
use argh::FromArgs;
use serde::Deserialize;
use warp::http::StatusCode;
use warp::{self, path, Filter};

use redarrow::dispatcher;

// The query parameters for command.
#[derive(Debug, Deserialize)]
pub struct CommandOptions {
    argument: Option<String>,
    chunked: Option<u8>,
}

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

#[tokio::main]
async fn main() {
    let args: ServerArgs = argh::from_env();

    let configs = dispatcher::read_config(args.config.as_str()).unwrap();

    let command = path!("command" / String)
        .and(warp::get())
        .and(warp::query::<CommandOptions>())
        .and(with_configs(configs))
        .and_then(handlers_command);

    warp::serve(command).run(([0, 0, 0, 0], args.port)).await;
}

pub async fn handlers_command(
    name: String,
    opts: CommandOptions,
    configs: dispatcher::Configs,
) -> Result<impl warp::Reply, Infallible> {
    match configs.get(name.as_str()) {
        None => {
            let err = dispatcher::CommandResult::err(format!("Unknown Command: {}", name));
            let json = warp::reply::json(&err);
            Ok(warp::reply::with_status(json, StatusCode::BAD_REQUEST))
        }
        Some(cmd) => {
            let arguments = match &opts.argument {
                None => Vec::new(),
                Some(args) => args.split(" ").collect(),
            };
            let r = cmd
                .execute(arguments)
                .unwrap_or_else(|err| dispatcher::CommandResult::err(format!("{}", err)));
            let json = warp::reply::json(&r);
            Ok(warp::reply::with_status(json, StatusCode::OK))
        }
    }
}

fn with_configs(
    configs: dispatcher::Configs,
) -> impl Filter<Extract = (dispatcher::Configs,), Error = Infallible> + Clone {
    warp::any().map(move || configs.clone())
}

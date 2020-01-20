use clap::{load_yaml, value_t};
use serde::Deserialize;
use warp::{self, path, Filter};

use redarrow::dispatcher;

// The query parameters for command.
#[derive(Debug, Deserialize)]
struct CommandOptions {
    argument: Option<String>,
    chunked: Option<u8>,
}

#[tokio::main]
async fn main() {
    let yaml = load_yaml!("cli.yml");
    let matches = clap::App::from(yaml).get_matches();

    let config_file = matches.value_of("config").unwrap().to_string();
    let port = value_t!(matches, "port", u16).unwrap_or(4205);
    // let debug = matches.is_present("debug");

    let configs = dispatcher::read_config(config_file.as_str()).unwrap();

    // GET /command/warp
    let command = path!("command" / String)
        .and(warp::query::<CommandOptions>())
        .map(
            move |name: String, opts: CommandOptions| match configs.get(name.as_str()) {
                Some(cmd) => {
                    let arguments = match &opts.argument {
                        None => Vec::new(),
                        Some(args) => {
                            args.split(" ").collect()
                        },
                    };
                    let r = cmd.execute(arguments).unwrap();
                    format!("{}: {:?}\n{:?}\n{:?}\n", name, opts, cmd, r)
                },
                None => format!("{} is unreviewed.\n", name),
            },
        );

    warp::serve(command).run(([0, 0, 0, 0], port)).await;
}

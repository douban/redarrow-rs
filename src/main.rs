#[macro_use]
extern crate clap;

mod client;

use std::sync::mpsc;
use std::sync::mpsc::{Receiver, Sender};
use std::thread;

use serde_json;

fn main() {
    let (opts, detail) = parse_args();
    let exit_code = remote_run_cmd(opts, detail);
    std::process::exit(exit_code);
}

fn parse_args() -> (client::Opts, bool) {
    let yaml = load_yaml!("cli.yml");
    let matches = clap::App::from(yaml).get_matches();

    let host = matches.value_of("host").unwrap().to_string();
    let port = value_t!(matches, "port", u32).unwrap_or(4205);
    let detail = matches.is_present("detail");

    let mut command: String = "".to_string();
    let mut arguments: Vec<String> = Vec::new();

    if matches.is_present("list") {
        command = "*LIST*".to_string();
    } else {
        if let Some(args) = matches.values_of("args") {
            for (index, value) in args.enumerate() {
                if index == 0 {
                    command = value.to_string();
                } else {
                    arguments.push(value.to_string());
                }
            }
        } else {
            std::process::exit({
                eprintln!("Error: command missing");
                2
            });
        }
    }
    (
        client::Opts {
            host: host,
            port: port,
            command: command,
            arguments: arguments,
        },
        detail,
    )
}

fn remote_run_cmd(opts: client::Opts, detail: bool) -> i32 {
    let exit_code: i32;
    let (tx, rx): (Sender<client::Result>, Receiver<client::Result>) = mpsc::channel();
    if opts.host.contains(",") {
        let child = thread::spawn(move || client::run_parallel(opts, tx));
        exit_code = print_multple_hosts_result(rx, detail);
        child.join().unwrap();
    } else {
        let child = thread::spawn(move || client::rt_run(opts, tx));
        exit_code = print_result(rx, detail);
        child.join().unwrap();
    }
    exit_code
}

fn print_result(rx: Receiver<client::Result>, detail: bool) -> i32 {
    let mut finishied = false;
    let mut ret = 0;
    while !finishied {
        let result = rx.recv().unwrap_or(client::Result {
            host: "".to_string(),
            fd: -1,
            line: "UnfinishedCmd".to_string(),
        });
        match result.fd {
            0 => {
                finishied = true;
                let v: serde_json::Value = serde_json::from_str(result.line.as_str()).unwrap();
                if detail {
                    eprintln!("{}", "=".repeat(40));
                    eprintln!("{}", serde_json::to_string_pretty(&v).unwrap());
                }
                if v["error"].is_null() {
                    ret = v["exit_code"].as_i64().unwrap() as i32;
                } else {
                    eprintln!("{}", v["error"]);
                    ret = 3;
                }
            }
            1 => print!("{}", result.line),
            2 => eprint!("{}", result.line),
            _ => {
                finishied = true;
                eprintln!("{}", result.line);
                ret = 2;
            }
        }
    }
    ret
}

fn print_multple_hosts_result(_rx: Receiver<client::Result>, _detail: bool) -> i32 {
    // if !codes.iter().all(|&x| x == 0) {
    //     exit_code = 1;
    // }
    0
}

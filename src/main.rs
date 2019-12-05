#[macro_use]
extern crate clap;

mod client;

use std::sync::mpsc;
use std::sync::mpsc::{Receiver, Sender};
use std::thread;

use serde_json::Value;

fn main() {
    use clap::App;

    let yaml = load_yaml!("cli.yml");
    let matches = App::from(yaml).get_matches();

    let host = matches.value_of("host").unwrap().to_string();
    let port = value_t!(matches, "port", u32).unwrap_or(4205);
    let detail = matches.is_present("detail");

    let mut command: String = "".to_string();
    let mut arguments: Vec<String> = Vec::new();
    if matches.is_present("list") {
        command = "*LIST*".to_string()
    } else {
        if let Some(args) = matches.values_of("args") {
            for (index, value) in args.enumerate() {
                if index == 0 {
                    command = value.to_string();
                } else {
                    arguments.push(value.to_string());
                }
            }
        }
    }

    if command == "" {
        std::process::exit({
            App::from(yaml).print_help().unwrap();
            println!("");
            2
        });
    }

    let exit_code = remote_run_cmd(host, port, command, arguments, detail);
    std::process::exit(exit_code);
}

fn remote_run_cmd(
    host: String,
    port: u32,
    command: String,
    arguments: Vec<String>,
    detail: bool,
) -> i32 {
    let exit_code: i32;
    if host.contains(",") {
        let hosts: Vec<String> = host.split(",").map(|s| s.to_string()).collect();
        let (tx, rx): (Sender<(String, i8, String)>, Receiver<(String, i8, String)>) =
            mpsc::channel();
        let child =
            thread::spawn(move || client::run_parallel(hosts, port, command, arguments, tx));
        exit_code = print_multple_hosts_result(rx);
        child.join().unwrap();
    } else {
        let (tx, rx): (Sender<(i8, String)>, Receiver<(i8, String)>) = mpsc::channel();
        let child = thread::spawn(move || client::rt_run(host, port, command, arguments, tx));
        exit_code = print_result(rx, detail);
        child.join().unwrap();
    }
    exit_code
}

fn print_result(rx: Receiver<(i8, String)>, detail: bool) -> i32 {
    let mut finishied = false;
    let mut ret = 0;
    while !finishied {
        let (fd, line) = rx.recv().unwrap_or((-1, String::from("UnfinishedCmd")));
        match fd {
            0 => {
                finishied = true;
                let v: Value = serde_json::from_str(line.as_str()).unwrap();
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
            1 => print!("{}", line),
            2 => eprint!("{}", line),
            _ => {
                finishied = true;
                eprintln!("{}", line);
                ret = 2;
            }
        }
    }
    ret
}

fn print_multple_hosts_result(rx: Receiver<(String, i8, String)>) -> i32 {
    // if !codes.iter().all(|&x| x == 0) {
    //     exit_code = 1;
    // }
    0
}

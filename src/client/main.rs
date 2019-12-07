#[macro_use]
extern crate clap;

use std::collections::BTreeMap;
use std::sync::mpsc;
use std::sync::mpsc::{Receiver, Sender};
use std::thread;

use serde_json;

use redarrow::webclient;

fn main() {
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

    let opts = webclient::Opts {
        host: host,
        port: port,
        command: command,
        arguments: arguments,
    };

    let exit_code = remote_run_cmd(opts, detail);
    std::process::exit(exit_code);
}

fn remote_run_cmd(opts: webclient::Opts, detail: bool) -> i32 {
    let exit_code: i32;
    let (tx, rx): (Sender<webclient::It>, Receiver<webclient::It>) = mpsc::channel();
    if opts.host.contains(",") {
        let child = thread::spawn(move || webclient::run_parallel(opts, tx));
        let exit_codes = print_multple_hosts_result(rx);
        if exit_codes.iter().all(|(_, exit_code)| *exit_code == 0) {
            exit_code = 0;
        } else {
            exit_code = 1;
        }
        child.join().unwrap();
    } else {
        let child = thread::spawn(move || webclient::rt_run(opts, tx));
        exit_code = print_result(rx, detail);
        child.join().unwrap();
    }
    exit_code
}

fn print_result(rx: Receiver<webclient::It>, detail: bool) -> i32 {
    let mut ret = 0;
    loop {
        let result = rx.recv().unwrap_or(webclient::It {
            host: "".to_string(),
            fd: 0,
            line: format!("{{\"error\": \"Command unfinished\"}}"),
        });
        match result.fd {
            0 => {
                let v: serde_json::Value = serde_json::from_str(result.line.as_str()).unwrap();
                if detail {
                    eprintln!("{}", "=".repeat(40));
                    eprintln!("{}", serde_json::to_string_pretty(&v).unwrap());
                }
                if v["error"].is_null() {
                    ret = v["exit_code"].as_i64().unwrap() as i32;
                } else {
                    eprintln!("Error: {}", v["error"]);
                    ret = 3;
                }
                break;
            }
            1 => print!("{}", result.line),
            2 => eprint!("{}", result.line),
            _ => {
                eprintln!("Unknown result: {:?}", result);
                break;
            }
        }
    }
    ret
}

fn print_multple_hosts_result(rx: Receiver<webclient::It>) -> BTreeMap<String, i32> {
    let mut metas: BTreeMap<String, i32> = BTreeMap::new();
    let mut output: BTreeMap<String, Vec<String>> = BTreeMap::new();
    loop {
        let result = rx.recv().unwrap_or(webclient::It {
            host: "".to_string(),
            fd: 0,
            line: format!("{{\"error\": \"All finished\"}}"),
        });
        if result.host == "" {
            break;
        }
        match result.fd {
            0 => {
                let v: serde_json::Value = serde_json::from_str(result.line.as_str()).unwrap();
                println!(">>>>> {} <<<<<", &result.host);
                if let Some(o) = output.get_mut(&result.host) {
                    for l in o {
                        print!("{}", l);
                    }
                }
                let exit_code: i32;
                if v["error"].is_null() {
                    exit_code = v["exit_code"].as_i64().unwrap() as i32;
                    println!(">>>>> {} returns {} <<<<<", result.host, exit_code);
                } else {
                    println!(">>>>> {} returns error: <<<<<", result.host);
                    eprint!("{}", v["error"]);
                    exit_code = -1;
                }
                print!("\n----------------------------------------\n");
                metas.insert(result.host, exit_code);
            }
            1 | 2 => {
                if let Some(o) = output.get_mut(&result.host) {
                    o.push(result.line);
                } else {
                    output.insert(result.host, vec![result.line]);
                }
            }
            _ => {
                eprintln!("Unknown result: {:?}", result);
                break;
            }
        }
    }
    let bad_hosts: BTreeMap<String, i32> = metas
        .iter()
        .filter(|(_, exit_code)| **exit_code != 0)
        .map(|(host, exit_code)| ((*host).to_string().clone(), *exit_code))
        .collect();
    println!(
        "{} hosts in total, {} are okay.",
        metas.len(),
        metas.len() - bad_hosts.len()
    );
    if bad_hosts.len() > 0 {
        println!("There is something wrong with these hosts:");
        for (host, exit_code) in &bad_hosts {
            println!("{}: {}", host, exit_code);
        }
    }
    metas
}

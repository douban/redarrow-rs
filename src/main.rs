#[macro_use]
extern crate clap;

#[macro_use]
extern crate serde_derive;
extern crate serde;
extern crate serde_json;

mod client;

use std::sync::mpsc;
use std::sync::mpsc::{Receiver, Sender};

use futures::executor::LocalPool;

fn main() {
    use clap::App;

    let yaml = load_yaml!("cli.yml");
    let matches = App::from(yaml).get_matches();

    let host = matches.value_of("host").unwrap();
    let port = value_t!(matches, "port", u32).unwrap_or(4205);

    let mut command: &str = "";
    let mut arguments: Vec<&str> = Vec::new();
    if matches.is_present("list") {
        command = "*LIST*"
    } else {
        if let Some(args) = matches.values_of("args") {
            for (index, value) in args.enumerate() {
                if index == 0 {
                    command = value
                } else {
                    arguments.push(value);
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

    let exit_code: i32;
    let mut pool = LocalPool::new();

    if host.contains(",") {
        let hosts: Vec<&str> = host.split(",").collect();
        let (tx, rx): (Sender<(String, i8, String)>, Receiver<(String, i8, String)>) = mpsc::channel();
        pool.run_until(client::run_parallel(hosts, port, command, arguments, tx));
        exit_code = print_multple_hosts_result(rx);
    } else {
        let (tx, rx): (Sender<(i8, String)>, Receiver<(i8, String)>) = mpsc::channel();
        pool.run_until(client::rt_run(host, port, command, arguments, tx));
        exit_code = print_result(rx);
    }

    std::process::exit(exit_code);
}


fn print_result(rx: Receiver<(i8, String)>) -> i32 {
    let mut finishied = false;
    let mut ret = 0;
    while !finishied {
        let (fd, line) = rx.recv().unwrap_or((-1, String::from("UnfinishedCmd")));
        match fd {
            0 => {finishied = true},
            1 => println!("{}", line),
            2 => eprintln!("{}", line),
            _ => {finishied = true},
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

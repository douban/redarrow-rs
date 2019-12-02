
extern crate hyper;

use std::io::{self, Write};

use hyper::rt::{self, Future, Stream};
use hyper::Client;


fn fetch_url(url: hyper::Uri) -> impl Future<Item=(), Error=()> {
    let client = Client::new();

    client
        .get(url)
        .and_then(|res| {
            println!("Response: {}", res.status());
            println!("Headers: {:#?}", res.headers());

            res.into_body().for_each(|chunk| {
                io::stdout().write_all(&chunk)
                    .map_err(|e| panic!("example expects stdout is open, error={}", e))
            })
        })
        .map(|_| {
            println!("\n\nDone.");
        })
        .map_err(|err| {
            eprintln!("Error {}", err);
        })
}

pub fn realtime_run_command(host: &str, port: u32, command: &str, arguments: Vec<&str>) -> i32 {
    println!("host: {}", host);
    println!("port: {}", port);
    println!("command: {}", command);
    for a in &arguments {
        println!("argument: {}", a);
    }

    let url = format!("http://{}:{}/command/{}", host, port, command)
        .parse()
        .unwrap();

    rt::run(fetch_url(url));

    0
}

pub fn remote_run_in_parallel(
    hosts: Vec<&str>,
    port: u32,
    command: &str,
    arguments: Vec<&str>,
) -> Vec<i32> {
    for h in &hosts {
        println!("host: {}", h);
    }
    println!("port: {}", port);
    println!("command: {}", command);
    for a in &arguments {
        println!("argument: {}", a);
    }
    [0].to_vec()
}

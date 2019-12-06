use std::str;
use std::sync::mpsc::Sender;

use hyper::rt::{self, Future, Stream};
use hyper::Client;
use url::form_urlencoded;

pub struct Result {
    pub host: String,
    pub fd: i8,
    pub line: String,
}

pub struct Opts {
    pub host: String,
    pub port: u32,
    pub command: String,
    pub arguments: Vec<String>,
}

impl Opts {
    fn build_url(self: &Self) -> String {
        let mut param_builder = form_urlencoded::Serializer::new(String::new());
        param_builder.append_pair("chunked", "1");
        if !self.arguments.is_empty() {
            param_builder.append_pair("argument", self.arguments.join(" ").as_str());
        }
        let param = param_builder.finish();

        format!(
            "http://{}:{}/command/{}?{}",
            self.host,
            self.port,
            self.command,
            param
        )
    }
}

fn parse_chunk(s: &str) -> (i8, &str) {
    let mut v = s.splitn(2, "> ");
    let fd: i8 = v.next().unwrap().parse().unwrap();
    let line = v.next().unwrap();
    (fd, line)
}

pub fn rt_run(opts: Opts, tx: Sender<Result>) {
    let url = opts.build_url().parse().unwrap();
    rt::run({
        let client = Client::new();
        client
            .get(url)
            .and_then(move |res| {
                res.into_body().for_each(move |chunk| {
                    let b = &chunk.into_bytes();
                    let (fd, line) = parse_chunk(str::from_utf8(b).unwrap());
                    tx.send(Result {
                        host: opts.host.clone(),
                        fd: fd,
                        line: line.to_string(),
                    })
                    .unwrap();
                    Ok(())
                })
            })
            .map_err(|err| {
                println!("Error: {}", err);
            })
    });
}

pub fn run_parallel(opts: Opts, _tx: Sender<Result>) {
    let hosts: Vec<&str> = opts.host.split(",").collect();
    for h in hosts {
        println!("host: {}", h);
    }
    println!("port: {}", opts.port);
    println!("command: {}", opts.command);
    for a in opts.arguments {
        println!("argument: {}", a);
    }
}

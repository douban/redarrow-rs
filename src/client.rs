use std::str;
use std::sync::mpsc::Sender;
use std::thread;

use curl::easy::Easy;
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
            self.host, self.port, self.command, param
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
    let mut easy = Easy::new();
    easy.url(opts.build_url().as_str()).unwrap();
    easy.write_function(move |data| {
        let (fd, line) = parse_chunk(str::from_utf8(data).unwrap());
        tx.send(Result {
            host: opts.host.clone(),
            fd: fd,
            line: line.to_string(),
        })
        .unwrap();
        Ok(data.len())
    })
    .unwrap();
    easy.perform().unwrap();
}

pub fn run_parallel(opts: Opts, tx: Sender<Result>) {
    let hosts: Vec<&str> = opts.host.split(",").collect();
    let mut children = Vec::new();
    for host in hosts {
        let opts = Opts {
            host: host.to_string(),
            port: opts.port,
            command: opts.command.clone(),
            arguments: opts.arguments.clone(),
        };
        let tx = tx.clone();
        let child = thread::spawn(move || rt_run(opts, tx));
        children.push(child);
    }
    for child in children {
        // TODO:(everpcpc) error handling
        let _ = child.join().unwrap();
    }
}

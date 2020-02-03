use std::str;
use std::sync::atomic::{AtomicI8, Ordering};
use std::sync::mpsc::Sender;
use std::thread;
use std::time::Duration;

use anyhow::Result;
use curl::easy::Easy;
use url::form_urlencoded;

use crate::dispatcher;

#[derive(Debug)]
pub struct It {
    pub host: String,
    pub fd: i8,
    pub line: String,
}

#[derive(Debug)]
pub struct Client {
    pub host: String,
    pub port: u32,
    pub command: String,
    pub arguments: Vec<String>,
}

impl Client {
    pub fn new(host: String, port: u32, command: String, arguments: Vec<String>) -> Client {
        Client {
            host: host,
            port: port,
            command: command,
            arguments: arguments,
        }
    }

    fn build_url(self: &Self, chunked: bool) -> String {
        let mut param_builder = form_urlencoded::Serializer::new(String::new());
        if chunked {
            param_builder.append_pair("chunked", "1");
        }
        if !self.arguments.is_empty() {
            param_builder.append_pair("argument", self.arguments.join(" ").as_str());
        }
        let param = param_builder.finish();
        format!(
            "http://{}:{}/command/{}?{}",
            self.host, self.port, self.command, param
        )
    }

    pub fn run_command(self: &Self) -> Result<dispatcher::CommandResult> {
        let mut dst = Vec::new();
        let mut easy = Easy::new();
        easy.connect_timeout(Duration::new(3, 0))?;
        easy.url(self.build_url(false).as_str())?;
        {
            let mut transfer = easy.transfer();
            transfer.write_function(|data| {
                dst.extend_from_slice(data);
                Ok(data.len())
            })?;
            transfer.perform()?;
        }
        let body = String::from_utf8(dst)?;
        let ret: dispatcher::CommandResult = serde_json::from_str(body.as_str())?;
        Ok(ret)
    }

    pub fn run_realtime(self: &Self, tx: Sender<It>) {
        let mut easy = Easy::new();
        easy.connect_timeout(Duration::new(3, 0)).unwrap();
        easy.url(self.build_url(true).as_str()).unwrap();
        let last_fd = std::sync::Arc::new(AtomicI8::new(-1));
        let mut tmp = "".to_string();
        {
            let mut transfer = easy.transfer();
            transfer
                .write_function(|data| {
                    let v = str::from_utf8(data).unwrap();
                    tmp.push_str(v);
                    if v.ends_with("\n") {
                        let (mut fd, line) = parse_chunk(tmp.as_str());
                        if fd < 0 {
                            fd = last_fd.load(Ordering::SeqCst);
                        } else {
                            last_fd.store(fd, Ordering::SeqCst);
                        }
                        tx.send(It {
                            host: self.host.clone(),
                            fd: fd,
                            line: line.to_string(),
                        })
                        .unwrap();
                        tmp.clear();
                    }
                    Ok(data.len())
                })
                .unwrap();
            transfer.perform().unwrap_or_else(|e| {
                tx.send(It {
                    host: self.host.clone(),
                    fd: 0,
                    line: format!("{{\"error\": \"{}\"}}", e),
                })
                .unwrap();
            })
        }
    }

    pub fn run_parallel(self: &Self, tx: Sender<It>) {
        let hosts: Vec<&str> = self.host.split(",").collect();
        let mut children = Vec::new();
        for host in hosts {
            let client = Client::new(
                host.to_string(),
                self.port,
                self.command.clone(),
                self.arguments.clone(),
            );
            let tx = tx.clone();
            let child = thread::spawn(move || client.run_realtime(tx));
            children.push(child);
        }
        for child in children {
            child.join().unwrap();
        }
    }
}

fn parse_chunk(s: &str) -> (i8, &str) {
    if s.starts_with("0> ") {
        (0, s.trim_start_matches("0> "))
    } else if s.starts_with("1> ") {
        (1, s.trim_start_matches("1> "))
    } else if s.starts_with("2> ") {
        (2, s.trim_start_matches("2> "))
    } else {
        (-1, s)
    }
}

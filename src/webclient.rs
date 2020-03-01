use std::str;
use std::sync::atomic::{AtomicI8, Ordering};
use std::sync::mpsc;
use std::time::Duration;

use anyhow::Result;
use curl::easy::Easy;
use url::form_urlencoded;

use crate::result::CommandResult;

const VERSION: &'static str = env!("CARGO_PKG_VERSION");

#[derive(Debug)]
pub struct It {
    pub fd: i8,
    pub line: String,
}

#[derive(Debug)]
pub struct Client {
    host: String,
    port: u32,
    command: String,
    arguments: Vec<String>,
    user_agent: String,
    connect_timeout: Duration,
}

impl Client {
    pub fn new(host: String, port: u32, command: String, arguments: Vec<String>) -> Client {
        Client {
            host: host,
            port: port,
            command: command,
            arguments: arguments,
            user_agent: format!("Redarrow-webclient/{}", VERSION),
            connect_timeout: Duration::new(3, 0),
        }
    }

    pub fn set_user_agent(self: &mut Self, ua: &str) {
        self.user_agent = format!("{}/{}", ua, VERSION);
    }

    pub fn set_connect_timeout(self: &mut Self, timeout: Duration) {
        self.connect_timeout = timeout;
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

    pub fn run_command(self: &Self) -> Result<CommandResult> {
        let mut dst = Vec::new();
        let mut easy = Easy::new();
        easy.useragent(self.user_agent.as_str())?;
        easy.connect_timeout(self.connect_timeout)?;
        easy.url(self.build_url(false).as_str())?;
        {
            let mut transfer = easy.transfer();
            transfer.write_function(|data| {
                dst.extend_from_slice(data);
                Ok(data.len())
            })?;
            transfer.perform()?;
        }
        Ok(serde_json::from_slice(&dst)?)
    }

    pub fn run_realtime(self: &Self, tx: mpsc::Sender<It>) -> Result<CommandResult> {
        let mut easy = Easy::new();
        easy.useragent(self.user_agent.as_str())?;
        easy.connect_timeout(self.connect_timeout)?;
        easy.url(self.build_url(true).as_str())?;

        let mut ret = "".to_string();
        {
            let last_fd = std::sync::Arc::new(AtomicI8::new(-1));
            let mut tmp = Vec::new();

            let mut transfer = easy.transfer();
            transfer.write_function(|data| {
                match data.last() {
                    None => {
                        return Ok(0);
                    }
                    Some(char) => {
                        tmp.extend_from_slice(data);
                        if *char != b'\n' {
                            return Ok(data.len());
                        }
                    }
                }
                let (mut fd, line) = parse_chunk(str::from_utf8(&tmp).unwrap());
                if fd == 0 {
                    ret.push_str(line);
                } else {
                    if fd == -1 {
                        fd = last_fd.load(Ordering::SeqCst);
                    } else {
                        last_fd.store(fd, Ordering::SeqCst);
                    }
                    tx.send(It {
                        fd: fd,
                        line: line.to_string(),
                    })
                    .unwrap();
                }
                tmp.clear();
                Ok(data.len())
            })?;
            transfer.perform()?;
        }

        Ok(serde_json::from_str(ret.as_str())?)
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

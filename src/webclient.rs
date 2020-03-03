use std::str;
use std::sync::mpsc;
use std::time::Duration;

use anyhow::Result;
use curl::easy::Easy;
use url::form_urlencoded;

use crate::result::CommandResult;

const VERSION: &'static str = env!("CARGO_PKG_VERSION");

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

    pub fn run_realtime(self: &Self, tx: mpsc::Sender<(i8, Vec<u8>)>) -> Result<CommandResult> {
        let mut easy = Easy::new();
        easy.useragent(self.user_agent.as_str())?;
        easy.connect_timeout(self.connect_timeout)?;
        easy.url(self.build_url(true).as_str())?;

        let mut ret: Vec<u8> = Vec::new();
        {
            let mut last_fd = -1;
            let mut tmp = Vec::new();

            let mut transfer = easy.transfer();
            transfer.write_function(|data| {
                let mut line_ends = false;
                match data.last() {
                    None => {
                        eprintln!("Nothing received");
                        return Ok(data.len());
                    }
                    Some(char) => {
                        if *char == b'\n' {
                            line_ends = true;
                        }
                    }
                }
                if last_fd >= 0 {
                    tmp.extend_from_slice(data);
                    if line_ends {
                        if tx.send((last_fd, tmp.clone())).is_err() {
                            eprintln!("ClientError: send result to std failed")
                        };
                        last_fd = -1;
                        tmp.clear();
                    }
                } else {
                    let fd = parse_fd(data);
                    if line_ends {
                        if fd == 0 {
                            ret.extend_from_slice(&data[3..]);
                        } else {
                            if tx.send((fd, data[3..].to_vec())).is_err() {
                                eprintln!("ClientError: send result to std failed")
                            };
                        }
                    } else {
                        tmp.extend_from_slice(&data[3..]);
                        last_fd = fd;
                    }
                }
                Ok(data.len())
            })?;
            transfer.perform()?;
        }

        if ret.len() == 0 {
            Ok(CommandResult::err("Command Unfinished".to_string()))
        } else {
            Ok(serde_json::from_slice(&ret)?)
        }
    }
}

fn parse_fd(s: &[u8]) -> i8 {
    if s.len() < 3 {
        -1
    } else {
        let (left, _) = s.split_at(3);
        match left {
            b"0> " => 0,
            b"1> " => 1,
            b"2> " => 2,
            _ => -1,
        }
    }
}

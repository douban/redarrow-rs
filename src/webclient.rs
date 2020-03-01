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

    pub fn run_realtime(self: &Self, tx: mpsc::Sender<(i8, String)>) -> Result<CommandResult> {
        let mut easy = Easy::new();
        easy.useragent(self.user_agent.as_str())?;
        easy.connect_timeout(self.connect_timeout)?;
        easy.url(self.build_url(true).as_str())?;

        let mut ret = "".to_string();
        {
            let mut last_fd = -1;
            let mut tmp = Vec::new();

            let mut transfer = easy.transfer();
            transfer.write_function(|data| {
                let mut line_ends = false;
                match data.last() {
                    None => {
                        return Ok(0);
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
                        let (_, line) = parse_chunk(str::from_utf8(&tmp).unwrap());
                        tx.send((last_fd, line.to_string())).unwrap();
                        last_fd = -1;
                        tmp.clear();
                    }
                } else {
                    let (fd, line) = parse_chunk(str::from_utf8(&data).unwrap());
                    if line_ends {
                        if fd == 0 {
                            ret.push_str(line);
                        } else {
                            tx.send((fd, line.to_string())).unwrap();
                        }
                    } else {
                        tmp.extend_from_slice(data);
                        last_fd = fd;
                    }
                }
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

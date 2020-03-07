use std::sync::mpsc;
use std::time::Duration;

use anyhow::Result;

use crate::{CommandParams, CommandResult};

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
    pub fn new(host: String, port: u32, command: String, arguments: Vec<String>) -> Self {
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

    fn build_url(self: &Self) -> String {
        format!(
            "http://{}:{}/command/{}",
            self.host, self.port, self.command
        )
    }

    fn get_arguments(self: &Self) -> Option<String> {
        if self.arguments.is_empty() {
            None
        } else {
            Some(self.arguments.join(" "))
        }
    }

    pub async fn run_command(self: &Self) -> Result<CommandResult> {
        let params = CommandParams {
            chunked: None,
            argument: self.get_arguments(),
        };
        let body = reqwest::Client::builder()
            .user_agent(self.user_agent.as_str())
            .connect_timeout(self.connect_timeout)
            .build()?
            .get(self.build_url().as_str())
            .query(&params)
            .send()
            .await?
            .bytes()
            .await?;
        Ok(serde_json::from_slice(&body)?)
    }

    pub async fn run_realtime(
        self: &Self,
        tx: mpsc::Sender<(i8, Vec<u8>)>,
    ) -> Result<CommandResult> {
        let params = CommandParams {
            chunked: Some(1),
            argument: self.get_arguments(),
        };
        let mut res = reqwest::Client::builder()
            .user_agent(self.user_agent.as_str())
            .connect_timeout(self.connect_timeout)
            .build()?
            .get(self.build_url().as_str())
            .query(&params)
            .send()
            .await?;

        let mut last_fd = -1;
        let mut tmp: Vec<u8> = Vec::new();
        while let Some(chunk) = res.chunk().await? {
            let mut line_ends = false;
            match chunk.last() {
                None => {
                    eprintln!("empty chunk received");
                    continue;
                }
                Some(char) => {
                    if *char == b'\n' {
                        line_ends = true;
                    }
                }
            }
            if last_fd >= 0 {
                tmp.extend_from_slice(&chunk);
                if line_ends {
                    tx.send((last_fd, tmp.clone()))?;
                    last_fd = -1;
                    tmp.clear();
                }
                continue;
            }
            let fd = parse_fd(&chunk);
            match fd {
                0 => {
                    return Ok(serde_json::from_slice(&chunk[3..])?);
                }
                1 | 2 => {
                    if line_ends {
                        tx.send((fd, chunk[3..].to_vec()))?;
                    } else {
                        tmp.extend_from_slice(&chunk[3..]);
                        last_fd = fd;
                    }
                }
                _ => {
                    eprintln!("Response Error: {:?}", chunk);
                }
            }
        }
        Ok(CommandResult::err("Command Unfinished".to_string()))
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

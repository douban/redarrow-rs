use std::collections::BTreeMap;
use std::str;
use std::sync::mpsc;
use std::time::Duration;

use anyhow::Result;

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

    fn build_url(self: &Self) -> String {
        format!(
            "http://{}:{}/command/{}",
            self.host, self.port, self.command
        )
    }

    fn get_arguments(self: &Self) -> String {
        self.arguments.join(" ")
    }

    pub async fn run_command(self: &Self) -> Result<CommandResult> {
        let mut params = BTreeMap::new();
        if !self.arguments.is_empty() {
            params.insert("argument", self.get_arguments());
        }

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
        let mut params = BTreeMap::new();
        params.insert("chunked", "1".to_string());
        if !self.arguments.is_empty() {
            params.insert("argument", self.get_arguments());
        }
        let mut res = reqwest::Client::builder()
            .user_agent(self.user_agent.as_str())
            .connect_timeout(self.connect_timeout)
            .build()?
            .get(self.build_url().as_str())
            .query(&params)
            .send()
            .await?;
        while let Some(chunk) = res.chunk().await? {
            let fd = parse_fd(&chunk);
            if fd == 0 {
                return Ok(serde_json::from_slice(&chunk[3..])?);
            } else if fd == -1 {
                return Ok(CommandResult::err(format!("Chunk error:{:?}", chunk)));
            } else {
                tx.send((fd, chunk[3..].to_vec()))?;
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

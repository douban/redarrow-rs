use std::collections::HashMap;
use std::path::Path;
use std::process;
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{anyhow, Result};
use glob::glob;
use ini::Ini;
use regex::{Captures, Regex};
use serde::{Deserialize, Serialize};

static RE_ARGS: &str = r"\$\{(\d+)\}";

pub type Configs = HashMap<String, Command>;

#[derive(Clone, Debug)]
pub struct Command {
    name: String,
    exec: String,
    args: Vec<Regex>,
    time_limit: u64,
}

impl Command {
    fn new(name: &str, exec: &str, args: Vec<Regex>, time_limit: u64) -> Command {
        Command {
            name: name.to_string(),
            exec: exec.to_string(),
            args: args,
            time_limit: time_limit,
        }
    }

    fn get_command(self: &Self, arguments: Vec<&str>) -> Result<String> {
        if arguments.len() != self.args.len() {
            return Err(anyhow!(
                "Illegal Argument: Got {} args ({} expected)",
                arguments.len(),
                self.args.len()
            ));
        }
        for (i, arg) in arguments.iter().enumerate() {
            if !&self.args[i].is_match(arg) {
                return Err(anyhow!("Illegal Argument: {}", arg));
            }
        }
        let exec = Regex::new(RE_ARGS)?
            .replace_all(&self.exec, |caps: &Captures| match caps.get(1) {
                None => "",
                Some(c) => {
                    let arg_idx = c.as_str().parse::<usize>().unwrap_or(0);
                    arguments[arg_idx]
                }
            })
            .into_owned();
        Ok(exec)
    }

    pub fn execute(self: &Self, arguments: Vec<&str>) -> Result<CommandResult> {
        let cmd = self.get_command(arguments)?;
        let args: Vec<&str> = cmd.split(" ").collect();

        let start = SystemTime::now();
        let out = process::Command::new(args[0]).args(&args[1..]).output()?;
        let duration = start.elapsed()?;

        Ok(CommandResult::ok(
            String::from_utf8(out.stdout)?,
            String::from_utf8(out.stderr)?,
            out.status.code().unwrap_or(-1),
            duration.as_secs_f64(),
            start.duration_since(UNIX_EPOCH)?.as_secs_f64(),
        ))
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct CommandResult {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stdout: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stderr: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub exit_code: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub time_cost: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start_time: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl CommandResult {
    pub fn ok(
        stdout: String,
        stderr: String,
        exit_code: i32,
        time_cost: f64,
        start_time: f64,
    ) -> CommandResult {
        CommandResult {
            stdout: Some(stdout),
            stderr: Some(stderr),
            exit_code: Some(exit_code),
            time_cost: Some(time_cost),
            start_time: Some(start_time),
            error: None,
        }
    }

    pub fn err(err: String) -> CommandResult {
        CommandResult {
            stdout: None,
            stderr: None,
            exit_code: None,
            time_cost: None,
            start_time: None,
            error: Some(err),
        }
    }
}

pub fn read_config(config_file: &str) -> Result<Configs> {
    let p = Path::new(config_file);
    let mut cmds: Configs = HashMap::new();

    if p.is_dir() {
        let dir = p.join("*").to_str().unwrap().to_string();
        for e in glob(dir.as_str())? {
            parse_config_file(e?, &mut cmds)?;
        }
    } else {
        parse_config_file(p, &mut cmds)?;
    }
    Ok(cmds)
}

fn parse_config_file<P: AsRef<Path>>(config_file: P, cmds: &mut Configs) -> Result<()> {
    let conf = Ini::load_from_file_noescape(config_file)?;

    for (sec, prop) in conf.iter() {
        let name = match sec {
            None => continue,
            Some(n) => n,
        };

        let exec = match prop.get("exec") {
            None => continue,
            Some(e) => e,
        };

        let mut args: Vec<Regex> = Vec::new();
        let re = Regex::new(RE_ARGS)?;
        for cap in re.captures_iter(exec) {
            let arg_name = format!("arg{}", cap.get(1).map_or("0", |m| m.as_str()));
            let arg = prop.get(arg_name.as_str()).unwrap();
            let arg_re = Regex::new(arg)?;
            args.push(arg_re);
        }

        let time_limit: u64 = match prop.get("time_limit") {
            Some(limit) => limit.parse()?,
            None => 30,
        };
        let cmd = Command::new(name, exec, args, time_limit);

        cmds.insert(name.to_string(), cmd);
    }
    Ok(())
}

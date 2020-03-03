use std::collections::HashMap;
use std::io::{BufRead, BufReader, Read};
use std::path::Path;
use std::process;
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use anyhow::{anyhow, Result};
use glob::glob;
use ini::Ini;
use nix::sys::signal;
use nix::unistd::Pid;
use regex::{Captures, Regex};
use wait_timeout::ChildExt;

use crate::result::CommandResult;

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

    // get a command with arguments
    fn get_command(self: &Self, arguments: Vec<String>) -> Result<(String, Vec<String>)> {
        if arguments.len() != self.args.len() {
            return Err(anyhow!(
                "Illegal Argument: Got {} args ({} expected)",
                arguments.len(),
                self.args.len()
            ));
        }
        for (i, arg) in arguments.iter().enumerate() {
            // NOTE: allow empty argument
            if arg == "" {
                continue;
            }
            if !&self.args[i].is_match(arg) {
                return Err(anyhow!("Illegal Argument: {}", arg));
            }
        }

        let mut cmd: &str = "";
        let mut args: Vec<String> = Vec::new();

        let re = Regex::new(RE_ARGS)?;

        let splited = shlex::split(self.exec.as_str())
            .ok_or(0)
            .map_err(|_| anyhow!("Split command error for {}", self.name))?;
        for (i, arg) in splited.iter().enumerate() {
            // first argument is command
            if i == 0 {
                cmd = arg;
                continue;
            }
            let a = re
                .replace_all(arg, |caps: &Captures| match caps.get(1) {
                    None => "".to_string(),
                    Some(c) => match c.as_str().parse::<usize>() {
                        Err(_) => {
                            log::warn!("parse arg index error for {}: {}", self.name, arg,);
                            "".to_string()
                        }
                        Ok(idx) => arguments[idx].clone(),
                    },
                })
                .into_owned();
            args.push(a.trim_matches('"').trim_matches('\'').to_string());
        }
        Ok((cmd.to_string(), args))
    }

    pub fn execute(self: &Self, arguments: Vec<String>) -> Result<CommandResult> {
        let (cmd, args) = self.get_command(arguments)?;

        let start = SystemTime::now();
        let mut child = process::Command::new(cmd)
            .args(args)
            .stdout(process::Stdio::piped())
            .stderr(process::Stdio::piped())
            .spawn()?;

        let timeout = Duration::from_secs(self.time_limit);
        let status = child.wait_timeout(timeout)?;

        match status {
            None => kill_child(&mut child),
            Some(s) => {
                let stdout = match child.stdout.as_mut() {
                    None => "".to_string(),
                    Some(out) => {
                        let mut ss = String::new();
                        out.read_to_string(&mut ss)?;
                        ss
                    }
                };
                let stderr = match child.stderr.as_mut() {
                    None => "".to_string(),
                    Some(err) => {
                        let mut ss = String::new();
                        err.read_to_string(&mut ss)?;
                        ss
                    }
                };

                Ok(match s.code() {
                    None => CommandResult::err("Terminated by signal".to_string()),
                    Some(code) => CommandResult::ok(
                        stdout,
                        stderr,
                        code,
                        start.elapsed()?.as_secs_f64(),
                        start.duration_since(UNIX_EPOCH)?.as_secs_f64(),
                    ),
                })
            }
        }
    }

    pub fn execute_iter(
        self: &Self,
        arguments: Vec<String>,
        tx: std::sync::mpsc::Sender<String>,
    ) -> Result<CommandResult> {
        let (cmd, args) = self.get_command(arguments)?;

        let start = SystemTime::now();

        let mut child = process::Command::new(&cmd)
            .args(args)
            .stdout(process::Stdio::piped())
            .stderr(process::Stdio::piped())
            .spawn()?;

        let stdout_reader = BufReader::new(child.stdout.take().ok_or(anyhow!("stdout error"))?);
        let out_tx = tx.clone();
        let stdout_child = thread::Builder::new()
            .name(format!("stdout sender: {}", &cmd))
            .spawn(move || {
                stdout_reader
                    .lines()
                    .filter_map(|line| line.ok())
                    .for_each(|line| match out_tx.send(format!("1> {}\n", line)) {
                        Err(_) => log::warn!("error sending to stdout: {}", line),
                        Ok(()) => {}
                    });
            })?;
        let stderr_reader = BufReader::new(child.stderr.take().ok_or(anyhow!("stderr error"))?);
        let err_tx = tx.clone();
        let stderr_child = thread::Builder::new()
            .name(format!("stderr sender: {}", &cmd))
            .spawn(move || {
                stderr_reader
                    .lines()
                    .filter_map(|line| line.ok())
                    .for_each(|line| match err_tx.send(format!("2> {}\n", line)) {
                        Err(_) => log::warn!("error sending to stderr: {}", line),
                        Ok(()) => {}
                    });
            })?;
        let timeout = Duration::from_secs(self.time_limit);
        let status = child.wait_timeout(timeout)?;

        match status {
            // FIXME:(everpcpc) stdout_child and stderr_child should be force terminated
            None => kill_child(&mut child),
            Some(s) => {
                stdout_child
                    .join()
                    .map_err(|e| anyhow!("stdout failed: {:?}", e))?;
                stderr_child
                    .join()
                    .map_err(|e| anyhow!("stderr failed: {:?}", e))?;
                Ok(match s.code() {
                    None => CommandResult::err("Terminated by signal".to_string()),
                    Some(code) => CommandResult::chunked_ok(
                        code,
                        start.elapsed()?.as_secs_f64(),
                        start.duration_since(UNIX_EPOCH)?.as_secs_f64(),
                    ),
                })
            }
        }
    }
}

fn kill_child(child: &mut process::Child) -> Result<CommandResult> {
    let pid = Pid::from_raw(child.id() as i32);
    signal::kill(pid, signal::SIGTERM).map_err(|e| anyhow!("Kill failed: {}", e))?;
    let one_sec = Duration::from_secs(1);
    Ok(match child.wait_timeout(one_sec)? {
        Some(s) => CommandResult::err(format!("Time Limit Exceeded: {}", s)),
        None => {
            signal::kill(pid, signal::SIGKILL).map_err(|e| anyhow!("Force kill failed: {}", e))?;
            CommandResult::err("Time Limit Exceeded: killed".to_string())
        }
    })
}

pub fn read_config(config_file: &str) -> Result<Configs> {
    let p = Path::new(config_file);
    let mut cmds: Configs = HashMap::new();

    if p.is_dir() {
        let d = p.join("*");
        let dir = d
            .to_str()
            .ok_or(0)
            .map_err(|_| anyhow!("Config dir error"))?;
        for e in glob(dir)? {
            parse_config_file(e?, &mut cmds)?;
        }
    } else {
        parse_config_file(p, &mut cmds)?;
    }
    Ok(cmds)
}

fn parse_config_file<P: AsRef<Path>>(config_file: P, cmds: &mut Configs) -> Result<()> {
    let conf = Ini::load_from_file_noescape(config_file)?;

    'outer: for (sec, prop) in conf.iter() {
        let name = match sec {
            None => continue,
            Some(n) => n,
        };

        let exec = match prop.get("exec") {
            None => continue,
            Some(e) => e,
        };
        // NOTE:(everpcpc) shell pipe not supported
        if exec.contains("|") {
            log::warn!("ignored command with pipe: {}", name);
            continue;
        }

        let mut args: Vec<Regex> = Vec::new();
        let re = Regex::new(RE_ARGS)?;
        for cap in re.captures_iter(exec) {
            let arg_name = format!("arg{}", cap.get(1).map_or("0", |m| m.as_str()));
            let arg = prop
                .get(arg_name.as_str())
                .ok_or(0)
                .map_err(|_| anyhow!("{} not found for {}", arg_name, name))?;

            let arg_re = match Regex::new(arg) {
                Ok(r) => r,
                Err(e) => {
                    log::error!("ignored error command {}: {}", name, e);
                    continue 'outer;
                }
            };
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_command() {
        let (cmd, args) = Command {
            name: "test".to_string(),
            exec: "sleep ${0}".to_string(),
            args: vec![Regex::new(r"[A-Za-z0-9._~:/?@!$&'()*+,=-]+").unwrap()],
            time_limit: 5,
        }
        .get_command(vec!["1".to_string()])
        .unwrap();
        assert_eq!(cmd, "sleep");
        assert_eq!(args, vec!["1"]);
    }

    #[test]
    fn test_get_command_with_quote() {
        let (cmd, args) = Command {
            name: "test".to_string(),
            exec: "echo ${0} \"${1}\"".to_string(),
            args: vec![Regex::new(r"\d+").unwrap(), Regex::new(r"[\d ]+").unwrap()],
            time_limit: 5,
        }
        .get_command(vec!["1".to_string(), "3 4".to_string()])
        .unwrap();
        assert_eq!(cmd, "echo");
        assert_eq!(args, vec!["1".to_string(), "3 4".to_string()]);

        let (cmd, args) = Command {
            name: "test".to_string(),
            exec: "echo \'${0}\' \'${1}\'".to_string(),
            args: vec![Regex::new(r"\w+").unwrap(), Regex::new(r"[\w ]+").unwrap()],
            time_limit: 5,
        }
        .get_command(vec!["1".to_string(), "34".to_string()])
        .unwrap();
        assert_eq!(cmd, "echo");
        assert_eq!(args, vec!["1", "34"]);
    }

    #[test]
    fn test_get_command_with_space() {
        let (cmd, args) = Command {
            name: "test".to_string(),
            exec: "echo -e \"${0} ${1}\" ${2}".to_string(),
            args: vec![
                Regex::new(r"\w+").unwrap(),
                Regex::new(r"[\w ]+").unwrap(),
                Regex::new(r"[\w ]+").unwrap(),
            ],
            time_limit: 5,
        }
        .get_command(vec!["1".to_string(), "4".to_string(), "8".to_string()])
        .unwrap();
        assert_eq!(cmd, "echo");
        assert_eq!(args, vec!["-e", "1 4", "8"]);
    }
}

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
        let mut child = process::Command::new(args[0])
            .args(&args[1..])
            .stdout(process::Stdio::piped())
            .stderr(process::Stdio::piped())
            .spawn()?;

        let timeout = Duration::from_secs(self.time_limit);
        let status = child.wait_timeout(timeout)?;

        match status {
            None => kill_child(&mut child),
            Some(s) => {
                let stdout = match child.stdout.as_mut() {
                    Some(out) => {
                        let mut ss = String::new();
                        out.read_to_string(&mut ss)?;
                        ss
                    }
                    None => "".to_string(),
                };
                let stderr = match child.stderr.as_mut() {
                    Some(err) => {
                        let mut ss = String::new();
                        err.read_to_string(&mut ss)?;
                        ss
                    }
                    None => "".to_string(),
                };

                Ok(CommandResult::ok(
                    stdout,
                    stderr,
                    s.code().unwrap_or(-1),
                    start.elapsed()?.as_secs_f64(),
                    start.duration_since(UNIX_EPOCH)?.as_secs_f64(),
                ))
            }
        }
    }

    pub fn execute_iter(
        self: &Self,
        arguments: Vec<&str>,
        tx: std::sync::mpsc::Sender<String>,
    ) -> Result<CommandResult> {
        let cmd = self.get_command(arguments)?;
        let args: Vec<&str> = cmd.split(" ").collect();

        let start = SystemTime::now();

        let mut child = process::Command::new(args[0])
            .args(&args[1..])
            .stdout(process::Stdio::piped())
            .stderr(process::Stdio::piped())
            .spawn()?;

        let stdout_reader = BufReader::new(child.stdout.take().ok_or(anyhow!("stdout error"))?);
        let out_tx = tx.clone();
        let stdout_child = thread::spawn(move || {
            stdout_reader
                .lines()
                .filter_map(|line| line.ok())
                .for_each(|line| {
                    out_tx.send(format!("1> {}\n", line)).unwrap();
                });
        });
        let stderr_reader = BufReader::new(child.stderr.take().ok_or(anyhow!("stderr error"))?);
        let err_tx = tx.clone();
        let stderr_child = thread::spawn(move || {
            stderr_reader
                .lines()
                .filter_map(|line| line.ok())
                .for_each(|line| {
                    err_tx.send(format!("2> {}\n", line)).unwrap();
                });
        });
        let timeout = Duration::from_secs(self.time_limit);
        let status = child.wait_timeout(timeout)?;

        match status {
            // FIXME:(everpcpc) stdout_child and stderr_child should be force terminated
            None => kill_child(&mut child),
            Some(s) => {
                stdout_child.join().unwrap();
                stderr_child.join().unwrap();
                Ok(CommandResult::chunked_ok(
                    s.code().unwrap_or(-1),
                    start.elapsed()?.as_secs_f64(),
                    start.duration_since(UNIX_EPOCH)?.as_secs_f64(),
                ))
            }
        }
    }
}

fn kill_child(child: &mut process::Child) -> Result<CommandResult> {
    let pid = Pid::from_raw(child.id() as i32);
    signal::kill(pid, signal::SIGTERM).map_err(|e| anyhow!("Kill failed: {}", e))?;
    let one_sec = Duration::from_secs(1);
    match child.wait_timeout(one_sec)? {
        Some(s) => Ok(CommandResult::err(format!("Time Limit Exceeded: {}", s))),
        None => {
            signal::kill(pid, signal::SIGKILL).map_err(|e| anyhow!("Force kill failed: {}", e))?;
            Ok(CommandResult::err(
                "Time Limit Exceeded: killed".to_string(),
            ))
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
            let arg = prop.get(arg_name.as_str()).unwrap();

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

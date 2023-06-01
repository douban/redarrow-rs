pub mod dispatcher;
pub mod webclient;

use prometheus::{TextEncoder, Encoder, Opts, Counter, Registry, Gauge};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
pub struct CommandParams {
    pub chunked: Option<u8>,
    pub argument: Option<String>,
    pub format: Option<String>,
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
    ) -> Self {
        CommandResult {
            stdout: Some(stdout),
            stderr: Some(stderr),
            exit_code: Some(exit_code),
            time_cost: Some(time_cost),
            start_time: Some(start_time),
            error: None,
        }
    }

    pub fn chunked_ok(exit_code: i32, time_cost: f64, start_time: f64) -> Self {
        CommandResult {
            stdout: None,
            stderr: None,
            exit_code: Some(exit_code),
            time_cost: Some(time_cost),
            start_time: Some(start_time),
            error: None,
        }
    }

    pub fn err(err: String) -> Self {
        CommandResult {
            stdout: None,
            stderr: None,
            exit_code: None,
            time_cost: None,
            start_time: None,
            error: Some(err),
        }
    }

    pub fn to_json(self: &Self) -> String {
        serde_json::to_string(self).unwrap()
    }

    pub fn to_prometheus(self: &Self) -> String {
        // Create a Counter.
        let command_success_opt = Opts::new("redarrow_command_success", "command result success, 1 for success, 0 for failed");
        let command_success = Gauge::with_opts(command_success_opt).unwrap();
        let command_return_code_opt = Opts::new("redarrow_command_return_code", "command return code");
        let command_return_code = Gauge::with_opts(command_return_code_opt).unwrap();
        let command_time_cost_opt = Opts::new("redarrow_command_time_cost", "command time cost");
        let command_time_cost = Gauge::with_opts(command_time_cost_opt).unwrap();

        let r = Registry::new();
        r.register(Box::new(command_success.clone())).unwrap();
        r.register(Box::new(command_return_code.clone())).unwrap();
        r.register(Box::new(command_time_cost.clone())).unwrap();

        // return  code = 0 is considered success
        // other case is considered failed
        if self.error.is_some() {
            command_success.set(0.0);
        } else {
            match self.exit_code {
                Some(code) => {
                    if code == 0 {
                        command_success.set(1.0);
                    } else {
                        command_success.set(0.0);
                    }
                    command_return_code.set(code as f64);
                    command_time_cost.set(self.time_cost.unwrap_or(0.0))
                }
                None => {
                    command_success.set(0.0);
                }
            }
        }

        // Gather the metrics.
        let mut buffer = vec![];
        let encoder = TextEncoder::new();
        let metric_families = r.gather();
        encoder.encode(&metric_families, &mut buffer).unwrap();

        // Output as string
        String::from_utf8(buffer).unwrap()
    }
}

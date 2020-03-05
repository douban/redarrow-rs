pub mod dispatcher;
pub mod webclient;

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
struct CommandParams {
    chunked: Option<String>,
    argument: Option<String>,
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
}

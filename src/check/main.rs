#[macro_use]
extern crate clap;

use std::f64::{INFINITY, NEG_INFINITY};

use redarrow::webclient;

/*
Threshold format: [@]start:end

Notes:
  1. start <= end
  2. start and ":" is not required if start=0
  3. if range is of format "start:" and end is not specified, assume end is
     infinity
  4. to specify negative infinity, use "~"
  5. alert is raised if metric is outside start and end range (inclusive of
     endpoints)
  6. if range starts with "@", then alert if inside this range (inclusive of
     endpoints)

For more detail and examples,
    see http://nagiosplug.sourceforge.net/developer-guidelines.html#THRESHOLDFORMAT
*/

struct Threshold {
    inside: bool,
    start: f64,
    end: f64,
}

impl Threshold {
    fn parse(range_str: &str) -> Threshold {
        let mut threshold = Threshold {
            inside: false,
            start: 0.0,
            end: INFINITY,
        };
        let thredshold_str: &str;
        if range_str.starts_with("@") {
            threshold.inside = true;
            thredshold_str = &range_str[1..];
        } else {
            thredshold_str = range_str;
        }
        if thredshold_str.contains(":") {
            let mut v = thredshold_str.splitn(2, ":");
            threshold.start = Threshold::parse_value(v.next().unwrap());
            threshold.end = Threshold::parse_value(v.next().unwrap());
        } else {
            threshold.end = Threshold::parse_value(thredshold_str);
        }
        threshold
    }

    fn parse_value(value: &str) -> f64 {
        if value == "~" {
            NEG_INFINITY
        } else if value == "" {
            INFINITY
        } else {
            value.parse().unwrap()
        }
    }
}

fn main() {
    let yaml = load_yaml!("cli.yml");
    let matches = clap::App::from(yaml).get_matches();

    let host = matches.value_of("host").unwrap().to_string();
    let command = matches.value_of("command").unwrap().to_string();

    let mut arguments: Vec<String> = Vec::new();
    if matches.is_present("arguments") {
        arguments = matches
            .value_of("arguments")
            .unwrap()
            .split(" ")
            .map(|x| x.to_string())
            .collect();
    }

    let quiet = matches.is_present("quiet");
    let opts = webclient::Opts::new(host, 4205, command, arguments);

    let ret: webclient::CommandResult;
    let result = webclient::run_command(opts);
    match result {
        Ok(v) => {
            ret = v;
            if ret.error != "" {
                if quiet {
                    std::process::exit(0);
                } else {
                    eprintln!("remote internal error: {}", ret.error);
                    std::process::exit(3);
                }
            }
        }
        Err(e) => {
            if quiet {
                std::process::exit(0);
            } else {
                eprintln!("local internal error: {}", e);
                std::process::exit(3);
            }
        }
    }

    if matches.is_present("raw") {
        let output: String;
        if ret.stdout != "" {
            output = ret.stdout;
        } else if ret.stderr != "" {
            output = ret.stderr;
        } else if ret.exit_code != 0 {
            output = format!("Error: exit code is {}", ret.exit_code);
        } else {
            output = "OK".to_string();
        }

        std::process::exit({
            println!("{}", output);
            ret.exit_code
        });
    }

    let value: f64 = ret.stdout.parse().unwrap();
    println!("{}", ret.stdout);

    let exit_code: i32;
    if matches.is_present("critical") && should_alert(value, matches.value_of("critical").unwrap())
    {
        exit_code = 2;
    } else if matches.is_present("warning")
        && should_alert(value, matches.value_of("warning").unwrap())
    {
        exit_code = 1;
    } else {
        exit_code = 0;
    }
    std::process::exit(exit_code);
}

fn should_alert(value: f64, range_str: &str) -> bool {
    let threshold = Threshold::parse(range_str);
    if threshold.inside {
        threshold.start <= value && value <= threshold.end
    } else {
        value < threshold.start || threshold.end < value
    }
}

use std::f64::{INFINITY, NEG_INFINITY};

use argh::FromArgs;

use redarrow::{result, webclient};

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

    fn should_alert(self: &Self, value: f64) -> bool {
        if self.inside {
            self.start <= value && value <= self.end
        } else {
            value < self.start || self.end < value
        }
    }
}

#[argh(description = "execute remote nagios check from a redarrow server")]
#[derive(FromArgs, Debug)]
struct CheckArgs {
    #[argh(positional)]
    host: String,

    #[argh(positional)]
    command: String,

    #[argh(option, short = 'a', description = "redarrow command arguments")]
    arguments: Option<String>,

    #[argh(option, short = 'w', description = "warning threshold")]
    warning: Option<String>,

    #[argh(option, short = 'c', description = "critical threshold")]
    critical: Option<String>,

    #[argh(
        switch,
        short = 'r',
        description = "use remote output and return code directly"
    )]
    raw: bool,

    #[argh(
        switch,
        short = 'q',
        description = "catch all redarrow run_command exceptions and not alert"
    )]
    quiet: bool,
}

fn main() {
    let args: CheckArgs = argh::from_env();

    let arguments: Vec<String> = match args.arguments {
        None => Vec::new(),
        Some(a) => a.split(" ").map(|x| x.to_string()).collect(),
    };

    let ret: result::CommandResult;

    let client = webclient::Client::new(args.host, 4205, args.command, arguments);
    let result = client.run_command();
    match result {
        Ok(v) => {
            ret = v;
            match ret.error {
                None => {}
                Some(err) => {
                    if args.quiet {
                        std::process::exit(0);
                    } else {
                        eprintln!("remote internal error: {}", err);
                        std::process::exit(3);
                    }
                }
            }
        }
        Err(e) => {
            if args.quiet {
                std::process::exit(0);
            } else {
                eprintln!("local internal error: {}", e);
                std::process::exit(3);
            }
        }
    }

    let stdout = ret.stdout.unwrap();
    let stderr = ret.stderr.unwrap();

    if args.raw {
        let output: String;

        if stdout != "" {
            output = stdout;
        } else if stderr != "" {
            output = stderr;
        } else if ret.exit_code.unwrap() != 0 {
            output = format!("Error: exit code is {}", ret.exit_code.unwrap());
        } else {
            output = "OK".to_string();
        }
        println!("{}", output);
        std::process::exit(ret.exit_code.unwrap());
    }

    println!("{}", stdout);
    let value: f64 = stdout.trim().parse().unwrap();

    match args.critical {
        None => {}
        Some(critical) => {
            let threshold = Threshold::parse(critical.as_str());
            if threshold.should_alert(value) {
                std::process::exit(2);
            }
        }
    }
    match args.warning {
        None => {}
        Some(warning) => {
            let threshold = Threshold::parse(warning.as_str());
            if threshold.should_alert(value) {
                std::process::exit(2);
            }
        }
    }

    std::process::exit(0);
}

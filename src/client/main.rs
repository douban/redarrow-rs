use std::collections::HashMap;
use std::sync::mpsc;
use std::thread;

use argh::FromArgs;

use redarrow::result::CommandResult;
use redarrow::webclient::{Client, It};

#[argh(description = "execute remote command from a redarrow server")]
#[derive(FromArgs, Debug)]
struct ClientArgs {
    #[argh(positional)]
    command: String,

    #[argh(positional)]
    arguments: Vec<String>,

    #[argh(switch, description = "output the detail information of running")]
    detail: bool,

    #[argh(
        option,
        default = r#""localhost".to_string()"#,
        description = "comma-seperated redarrow service hosts"
    )]
    host: String,

    #[argh(option, default = "4205", description = "redarrow service port")]
    port: u32,
}

fn main() {
    let args: ClientArgs = argh::from_env();

    let exit_code: i32;

    if args.host.contains(",") {
        exit_code = run_parallel(args);
    } else {
        exit_code = run_single(args);
    }

    std::process::exit(exit_code);
}

fn run_single(args: ClientArgs) -> i32 {
    let mut client = Client::new(args.host, args.port, args.command, args.arguments);
    client.set_user_agent("Redarrow-client");
    let (tx, rx) = mpsc::channel::<It>();
    // NOTE: will not join this thread
    let _child = thread::spawn(move || loop {
        match rx.recv() {
            Err(_) => eprintln!("Recv Error!"),
            Ok(result) => match result.fd {
                0 => break,
                1 => print!("{}", result.line),
                2 => eprint!("{}", result.line),
                _ => {
                    eprintln!("Unknown result: {:?}", result);
                }
            },
        }
    });
    let exit_code = match client.run_realtime(tx.clone()) {
        Err(e) => {
            eprintln!("ClientError: {}", e);
            3
        }
        Ok(ret) => {
            if args.detail {
                eprintln!("{}", "=".repeat(40));
                eprintln!("{}", serde_json::to_string_pretty(&ret).unwrap());
            }
            match ret.error {
                None => ret.exit_code.unwrap(),
                Some(err) => {
                    eprintln!("ServerError: {}", err);
                    3
                }
            }
        }
    };
    tx.send(It {
        fd: 0,
        line: "".to_string(),
    })
    .unwrap_or_else(|_| {
        eprintln!("Printer Unexpectedly Exited!",);
    });
    exit_code
}

fn run_parallel(args: ClientArgs) -> i32 {
    let mut children = Vec::new();
    let (tx, rx) = mpsc::channel::<(String, CommandResult)>();

    for host in args.host.split(",") {
        let host = host.to_string();
        let tx = tx.clone();
        let mut client = Client::new(
            host.clone(),
            args.port,
            args.command.clone(),
            args.arguments.clone(),
        );
        client.set_user_agent("Redarrow-client");

        let child = thread::spawn(move || match client.run_command() {
            Ok(ret) => tx.send((host, ret)).unwrap(),
            Err(e) => tx
                .send((host, CommandResult::err(format!("ClientError: {}", e))))
                .unwrap(),
        });
        children.push(child);
    }

    let mut count = 0;
    let mut metas: HashMap<String, i32> = HashMap::new();
    loop {
        if count == children.len() {
            break;
        }
        match rx.recv() {
            Err(_) => eprintln!("Recv Error!"),
            Ok((host, ret)) => {
                count += 1;
                println!(">>>>> {} <<<<<", host);
                match ret.error {
                    Some(err) => {
                        println!(">>>>> {} returns error: <<<<<", host);
                        eprint!("{}", err);
                        metas.insert(host, 3);
                    }
                    None => {
                        print!("{}", ret.stdout.unwrap());
                        eprint!("{}", ret.stderr.unwrap());
                        println!(">>>>> {} returns {} <<<<<", host, ret.exit_code.unwrap());
                        metas.insert(host, ret.exit_code.unwrap());
                    }
                };
                print!("\n----------------------------------------\n");
            }
        }
    }

    for child in children {
        child.join().unwrap();
    }

    let bad_hosts: HashMap<String, i32> = metas
        .iter()
        .filter(|(_, exit_code)| **exit_code != 0)
        .map(|(host, exit_code)| ((*host).to_string().clone(), *exit_code))
        .collect();
    println!(
        "{} hosts in total, {} are okay.",
        metas.len(),
        metas.len() - bad_hosts.len()
    );
    if bad_hosts.len() > 0 {
        println!("There is something wrong with these hosts:");
        for (host, exit_code) in bad_hosts {
            println!("{}: {}", host, exit_code);
        }
    }

    if metas.iter().all(|(_, exit_code)| *exit_code == 0) {
        0
    } else {
        1
    }
}

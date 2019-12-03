use std::str;
use std::sync::mpsc::Sender;

use hyper::rt::{self, Future, Stream};
use hyper::Client;
use url::form_urlencoded;


#[derive(Serialize, Deserialize, Debug, Default)]
struct Result<'a> {
    fd: u8,
    stdout: &'a str,
    stderr: &'a str,
    time_cost: f64,
    start_time: f64,
    exit_code: i32,
    error: String,
}

fn build_url(host: &str, port: u32, command: &str, arguments: Vec<&str>) -> String {
    let mut param_builder = form_urlencoded::Serializer::new(String::new());
    param_builder.append_pair("chunked", "1");
    if !arguments.is_empty() {
        param_builder.append_pair("argument", arguments.join(" ").as_str());
    }
    let param = param_builder.finish();

    format!(
        "http://{}:{}/command/{}?{}",
        host,
        port,
        command,
        param.as_str()
    )
}

fn parse_chunk(s: &str) -> (i8, &str) {
    let mut v = s.splitn(2, "> ");
    let fd: i8 = v.next().unwrap().parse().unwrap();
    let line = v.next().unwrap();
    (fd, line)
}

// {"time_cost": 0.029803991317749023, "start_time": 1575272652.624216, "exit_code": 0, "stderr": "", "stdout": " 15:44:12 up 105 days, 23:41,  8 users,  load average: 7.77, 10.36, 6.26\n"}

pub async fn rt_run(host: &str, port: u32, command: &str, arguments: Vec<&str>, tx: Sender<(i8, String)>) -> () {
    let url = build_url(host, port, command, arguments).parse().unwrap();
    rt::run({
        let client = Client::new();
        client
            .get(url)
            .and_then(move |res| {
                res.into_body().for_each(move |chunk| {
                    let b = &chunk.into_bytes();
                    let (fd, line) = parse_chunk(str::from_utf8(b).unwrap());
                    tx.send((fd, String::from(line))).unwrap();
                    Ok(())
                })
            })
            .map_err(|err| {
                println!("Error: {}", err);
            })
    });
}

pub async fn run_parallel(hosts: Vec<&str>, port: u32, command: &str, arguments: Vec<&str>, tx: Sender<(String, i8, String)>) -> () {
    for h in &hosts {
        println!("host: {}", h);
    }
    println!("port: {}", port);
    println!("command: {}", command);
    for a in &arguments {
        println!("argument: {}", a);
    }
}

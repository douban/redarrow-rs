
pub fn realtime_run_command(host: &str, port: u32, command: &str, arguments: Vec<&str>) -> i32 {
    println!("host: {}", host);
    println!("port: {}", port);
    println!("command: {}", command);
    for a in &arguments {
        println!("argument: {}", a);
    }
    0
}

pub fn remote_run_in_parallel(hosts: Vec<&str>, port: u32, command: &str, arguments: Vec<&str>) -> Vec<i32>{
    for h in &hosts {
        println!("host: {}", h);
    }
    println!("port: {}", port);
    println!("command: {}", command);
    for a in &arguments {
        println!("argument: {}", a);
    }
    [0].to_vec()
}

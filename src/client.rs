
pub fn run_remote_command(host: &str, port: u32, command: &str, arguments: Vec<&str>) -> i32{
    println!("host: {}", host);
    println!("port: {}", port);
    println!("command: {}", command);
    for a in &arguments {
        println!("argument: {}", a);
    }
    0
}
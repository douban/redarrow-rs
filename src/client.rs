
pub fn run_remote_command(host: String, port: u32, command: String, arguments: Vec<String>) -> i32{
    println!("host: {}", host);
    println!("port: {}", port);
    println!("command: {}", command);
    for a in &arguments {
        println!("argument: {}", a);
    }
    0
}
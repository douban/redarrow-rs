#[macro_use]
extern crate clap;

fn main() {
    use clap::App;

    let yaml = load_yaml!("cli.yml");
    let matches = App::from(yaml).get_matches();

    let host = matches.value_of("host").unwrap();
    let port = value_t!(matches, "port", u32).unwrap_or(4205);

    let mut command = String::new();
    let mut arguments: Vec<String> = Vec::new();
    if matches.is_present("list") {
        command.push_str("*LIST*")
    } else {
        if let Some(args) = matches.values_of("args") {
            for (index, value) in args.enumerate() {
                if index == 0 {
                    command.push_str(value)
                } else {
                    arguments.push(String::from(value));
                }
            }
        }
    }

    println!("host: {}", host);
    println!("port: {}", port);
    println!("command: {}", command);
    for a in &arguments {
        println!("argument: {}", a);
    }

}

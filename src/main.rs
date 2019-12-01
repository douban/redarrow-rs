#[macro_use]
extern crate clap;

mod client;

fn main() {
    use clap::App;

    let yaml = load_yaml!("cli.yml");
    let matches = App::from(yaml).get_matches();

    let host = value_t!(matches, "host", String).unwrap();
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

    let exit_code = client::run_remote_command(host,port,command,arguments);
    std::process::exit(exit_code);
}

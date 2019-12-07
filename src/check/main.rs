#[macro_use]
extern crate clap;

// use redarrow::webclient;

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

    println!("host {}", host);
    println!("command {}", command);
    println!("arguments {:?}", arguments);
}

#[macro_use]
extern crate clap;

fn main() {
    use clap::App;

    let yaml = load_yaml!("cli.yml");
    App::from(yaml).get_matches();

}

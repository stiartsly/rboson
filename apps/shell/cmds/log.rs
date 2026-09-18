use boson::core::logger;
use clap::{arg, ArgMatches, Command};

pub(crate) fn command() -> Command {
    Command::new("log")
        .about("Enable or disable console log output; logs are always written to the file")
        .arg(
            arg!([STATE] "Console logging state: on or off")
                .default_value("on")
                .value_parser(["on", "off"]),
        )
}

pub(crate) fn run(matches: &ArgMatches) {
    match matches.get_one::<String>("STATE").map(String::as_str) {
        Some("off") => {
            logger::disable_console_output();
            println!("Console log output disabled. Logs continue in the configured log file.");
        }
        Some("on") => {
            logger::enable_console_output();
            println!("Console log output enabled. Logs continue in the configured log file.");
        }
        _ => unreachable!("clap restricts log state to 'on' or 'off'"),
    }
}

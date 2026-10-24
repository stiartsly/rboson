// Copyright (c) 2023 -      bosonnetwork.io
//
// Permission is hereby granted, free of charge, to any person obtaining a copy
// of this software and associated documentation files (the "Software"), to deal
// in the Software without restriction, including without limitation the rights
// to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
// copies of the Software, and to permit persons to whom the Software is
// furnished to do so, subject to the following conditions:
//
// The above copyright notice and this permission notice shall be included in all
// copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
// IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
// FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
// AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
// LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
// OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
// SOFTWARE.

use boson::core::logger;
use clap::{arg, ArgMatches, Command};

pub(crate) fn command() -> Command {
    Command::new("log")
        .about("Enable or disable console log output")
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
            println!("Console log output disabled.");
        }
        Some("on") => {
            logger::enable_console_output();
            println!("Console log output enabled.");
        }
        _ => unreachable!("clap restricts log state to 'on' or 'off'"),
    }
}

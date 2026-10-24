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

use boson::dht::Node;
use clap::{arg, value_parser, ArgMatches, Command};

pub(crate) fn command() -> Command {
    Command::new("peer")
        .about("Look up peers announced under an id")
        .arg(arg!(<ID> "Target peer id (base58)"))
        .arg(
            arg!(-c --count <COUNT> "Expected number of peers")
                .default_value("8")
                .value_parser(value_parser!(usize)),
        )
}

pub(crate) async fn run(matches: &ArgMatches, node: &Node) {
    let id_str = matches.get_one::<String>("ID").unwrap();
    let Ok(peerid) = id_str.parse() else {
        red_print!("Invalid id '{id_str}'");
        return;
    };

    let count = *matches.get_one::<usize>("count").unwrap();
    println!("Attempting to find peers with id: {peerid} ...");
    let peer = match node.find_peer(&peerid, -1, count, None).await {
        Ok(v) => v,
        Err(e) => {
            red_print!("error: {}", e);
            return;
        }
    };

    if peer.is_empty() {
        green_println!("Found no peers !!!");
    } else {
        green_println!("Found {} peers, listed below: ", peer.len());
        for (i, item) in peer.iter().enumerate() {
            green_println!("peer [{}]: {}", i, item);
        }
    }
}

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
use clap::Command;

pub(crate) fn command() -> Command {
    Command::new("id")
        .alias("info")
        .about("Show information and status of the current DHT node")
        .visible_alias("status")
}

pub(crate) fn run(node: &Node, connected: bool) {
    println!("+------------------------------------------------------------+");
    println!("|                   Boson DHT Node Info                      |");
    println!("+------------------------------------------------------------+");
    println!("Node ID:        {}", node.id());
    println!("Running:        {}", if node.is_running() { "yes" } else { "no" });
    println!(
        "DHT Connection: {}",
        if connected { "connected" } else { "disconnected" }
    );
    match node.node_info() {
        Ok(node_info) => println!("Node Info:      {node_info}"),
        Err(e) => red_print!("Unable to read node information: {e}"),
    }
    println!("Data Directory: {}", node.options().data_dir());
    println!("Listen Port:    {}", node.options().port());
    if let Some(h4) = node.options().host4() {
        println!("IPv4 Host:      {h4}");
    }
    if let Some(h6) = node.options().host6() {
        println!("IPv6 Host:      {h6}");
    }
    println!(
        "Dev Mode:       {}",
        if node.options().developer_mode() {
            "enabled"
        } else {
            "disabled"
        }
    );
}

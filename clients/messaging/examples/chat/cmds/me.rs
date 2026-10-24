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

use std::sync::Arc;
use clap::{ArgMatches, Command};
use boson::messaging::Client;

pub(crate) fn cli() -> Command {
    Command::new("me")
        .about("Show user and client information")
        .help_template("{subcommands}")
}

fn connection_status(client: &Arc<Client>) -> String {
    if client.is_ready() {
        "Connected (Ready)".to_string()
    } else if client.is_connected() {
        "Connected".to_string()
    } else if client.is_running() {
        "Connecting".to_string()
    } else {
        "Disconnected".to_string()
    }
}

pub(crate) async fn execute(_: &ArgMatches, client: &Arc<Client>) {
    println!("User Id:            {}", client.user_id());
    println!("Device Id:          {}", client.device_id());
    println!("Messaging Peer Id:  {}", client.peer_id());
    println!(
        "Messaging endpoint: {}",
        client.peer_endpoint().as_str()
    );
    println!("Connection Status:  {}", connection_status(client));
}

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

use clap::{arg, ArgMatches, Command};
use boson::{
    dht::Node,
    signature::{KeyPair, PrivateKey},
    PeerInfo,
};

pub(crate) const DEFAULT_ENDPOINT: &str = "www.example.com";

pub(crate) fn command() -> Command {
    Command::new("peer")
        .visible_alias("announce_peer")
        .about("Announce a peer to the Boson network")
        .arg(arg!([ENDPOINT] "Endpoint value for the announced peer").default_value(DEFAULT_ENDPOINT))
        .arg(
            arg!(-k --key <PRIVATE_KEY> "Private key (hex or base58) for the peer identity; defaults to this node's own key")
                .required(false),
        )
        .arg(
            arg!(--seq <SEQUENCE_NUMBER> "Sequence number for the announced peer")
                .required(false),
        )
}

pub(crate) async fn run(matches: &ArgMatches, node: &Node, node_key: &PrivateKey) {
    let endpoint = matches
        .get_one::<String>("ENDPOINT")
        .map(String::as_str)
        .unwrap_or(DEFAULT_ENDPOINT);

    let Ok(sk) = matches.get_one::<String>("key")
        .map(|s| PrivateKey::try_from(s.as_str()))
        .transpose() else {
            red_print!("Invalid private key");
            return;
    };

    let kp = KeyPair::from(sk.unwrap_or(node_key.clone()));
    announce(node, endpoint, kp).await;
}

pub(crate) async fn announce(
    node: &Node,
    endpoint: &str,
    keypair: KeyPair,
) {
    let peer = match PeerInfo::builder(endpoint).with_key(keypair).build() {
        Ok(p) => p,
        Err(e) => {
            red_print!("Building peer info failed: {e}");
            return;
        }
    };

    println!(
        "Announcing peer {} with endpoint '{}' ...",
        peer.id(),
        peer.endpoint()
    );

    match node.announce_peer(&peer, -1, false).await {
        Ok(_) => green_println!("Peer {} announced successfully.", peer.id()),
        Err(e) => red_print!(
            "Failed to announce peer {}: {}",
            peer.id(),
            e
        ),
    }
}

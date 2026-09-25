use boson::{
    dht::Node,
    signature::{KeyPair, PrivateKey},
    PeerInfo,
};
use clap::{arg, ArgMatches, Command};

pub(crate) const DEFAULT_ENDPOINT: &str = "www.example.com";

pub(crate) fn command() -> Command {
    Command::new("announcepeer")
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

    let keypair = match matches.get_one::<String>("key") {
        Some(s) => match PrivateKey::try_from(s.as_str()) {
            Ok(sk) => KeyPair::from(sk),
            Err(e) => {
                println!("Invalid private key: {e}");
                return;
            }
        },
        _ => KeyPair::from(node_key.clone()),
    };

    announce(node, endpoint, keypair).await;
}

pub(crate) async fn announce(
    node: &Node,
    endpoint: &str,
    keypair: KeyPair,
) {
    let peer = match PeerInfo::builder(endpoint).with_key(keypair).build() {
        Ok(p) => p,
        Err(e) => {
            println!("Building peer info failed: {e}");
            return;
        }
    };

    println!(
        "Announcing peer {} with endpoint '{}' ...",
        peer.id(),
        peer.endpoint()
    );

    match node.announce_peer(&peer, -1, false).await {
        Ok(_) => println!("\x1b[32mPeer {} announced successfully.\x1b[0m", peer.id()),
        Err(e) => println!(
            "\x1b[31mFailed to announce peer {}: {}\x1b[0m",
            peer.id(),
            e
        ),
    }
}

use boson::{
    dht::Node,
    signature::{KeyPair, PrivateKey},
    PeerInfo,
};
use clap::{arg, ArgMatches, Command};

/// Default endpoint announced when the `announce_peer` command is run
/// without an explicit endpoint argument.
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
}

pub(crate) async fn run(matches: &ArgMatches, node: &Node, default_key: &PrivateKey) {
    let endpoint = matches
        .get_one::<String>("ENDPOINT")
        .map(String::as_str)
        .unwrap_or(DEFAULT_ENDPOINT);
    let key = matches.get_one::<String>("key").map(String::as_str);
    announce(node, endpoint, key, default_key).await;
}

/// Builds a peer info for `endpoint` and announces it to the network through
/// `node`.
///
/// The peer's identity key is `key_override` (hex or base58) when given,
/// otherwise `default_key` (this node's own configured private key) is
/// reused. Every other peer info field (fingerprint, sequence number, extra
/// data) is left at its default value.
pub(crate) async fn announce(
    node: &Node,
    endpoint: &str,
    key_override: Option<&str>,
    default_key: &PrivateKey,
) {
    let keypair = match key_override {
        Some(raw) => match PrivateKey::try_from(raw) {
            Ok(sk) => KeyPair::from(sk),
            Err(e) => {
                println!("Invalid private key: {e}");
                return;
            }
        },
        _ => KeyPair::from(default_key),
    };

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

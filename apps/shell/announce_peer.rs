use boson::{
    PeerInfo,
    dht::Node,
    signature::{KeyPair, PrivateKey},
};

/// Default endpoint announced when the `announce_peer` command is run
/// without an explicit endpoint argument.
pub(crate) const DEFAULT_ENDPOINT: &str = "www.example.com";

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
        None => KeyPair::from(default_key),
    };

    let peer = match PeerInfo::builder(endpoint).with_key(keypair).build() {
        Ok(p) => p,
        Err(e) => {
            println!("Building peer info failed: {e}");
            return;
        }
    };

    println!("Announcing peer {} with endpoint '{}' ...", peer.id(), peer.endpoint());
    match node.announce_peer(&peer, -1, true).await {
        Ok(_) => println!("\x1b[32mPeer announced successfully.\x1b[0m"),
        Err(e) => println!("\x1b[31mFailed to announce peer: {}\x1b[0m", e),
    }
}


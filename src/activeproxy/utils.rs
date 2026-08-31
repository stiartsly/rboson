use std::{
    sync::Arc,
    fs::File,
    io::Write,
    path::Path
};
use rand::seq::SliceRandom;
use log::{debug, warn};

use crate::{
    Id,
    PeerInfo,
    dht::Node,
    Result,
    errors::StateError,
};

pub(crate) async fn lookup_peer(node: Arc<Node>, peerid: &Id) -> Result<Option<PeerInfo>> {
    debug!("ActiveProxy is trying to find peer {} via DHT network...", peerid);

    let result = node.find_peer(peerid, -1, 4, None).await;
    if let Err(e) = result {
        return Err(StateError::new(format!("Trying to find peer but error: {}", e)));
    }

    let mut peers = result.unwrap();
    if peers.is_empty() {
        return Err(StateError::new(format!(
            "No peers with peerid {} is found at this moment, please try it later!!!",
            peerid)));
    }

    Ok({
        peers.shuffle(&mut rand::rng());
        peers.pop()
    })
}

pub(crate) fn save_peer(path: &Path, peer: &PeerInfo) {
    debug!("ActiveProxy is trying to persist peer {} into cached file...", peer.id());

    let mut buf = vec![];
    if let Err(e) = serde_json::to_writer(&mut buf, peer) {
        warn!("Failed to serialize peer {} error {e}", peer.id());
        return;
    }

    _ = File::create(path).map(|mut fp| {
        _ = fp.write_all(&buf);
        debug!("ActiveProxy persisted peer {} to cached file.", peer.id());
    });
}
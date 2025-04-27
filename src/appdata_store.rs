use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use log::{debug, info, warn};
use rand::seq::SliceRandom;
use rand::thread_rng;
use ciborium::value::Value as CVal;
use std::io::{Read, Write};
use std::fs::File;
use tokio::task;

use crate::{
    core::cbor,
    Id,
    Node,
    PeerInfo,
    NodeInfo,
    error::Result,
    Error
};

#[allow(unused)]
pub struct AppDataStoreBuilder<'a> {
    app_name: &'a str,
    node: Option<&'a Arc<Mutex<Node>>>,
    path: Option<&'a str>,
    peerid: Option<&'a Id>,
}

impl<'a> AppDataStoreBuilder<'a> {
    pub fn new(app_name: &'a str) -> Self {
        Self {
            app_name,
            node: None,
            path: None,
            peerid: None,
        }
    }

    pub fn with_node(&mut self, node: &'a Arc<Mutex<Node>>) -> &mut Self {
        self.node = Some(node);
        self
    }

    pub fn with_path(&mut self, path: &'a str) -> &mut Self {
        self.path = Some(path);
        self
    }

    pub fn with_peerid(&mut self, peerid: &'a Id) -> &mut Self {
        self.peerid = Some(peerid);
        self
    }

    pub fn build(&self) -> Result<AppDataStore> {
        let Some(node) = self.node else {
            return Err(Error::Argument("Missing docking DHT node!!!".into()))
        };
        let Some(path) = self.path else {
            return Err(Error::Argument("Missing storage path!!!".into()));
        };
        let Some(peerid) = self.peerid else {
            return Err(Error::Argument("Missing service peer Id!!!".into()));
        };

        Ok(AppDataStore::new(self.app_name, node, peerid, path))
    }
}

#[allow(unused)]
pub struct AppDataStore {
    app_name: String,
    node: Arc<Mutex<Node>>,
    path: PathBuf,
    peerid: Id,

    service_peer: Option<PeerInfo>,
    service_node: Option<NodeInfo>
}

#[allow(unused)]
impl AppDataStore {
    fn new(app: &str, node: &Arc<Mutex<Node>>, peerid: &Id, path: &str) -> Self {
        Self {
            app_name: app.to_string(),
            node    : node.clone(),
            peerid  : peerid.clone(),
            path    : PathBuf::from(path),
            service_peer: None,
            service_node: None
        }
    }

    pub fn store_path(&self) -> &str {
        self.path.to_str().unwrap_or(".")
    }

    pub fn service_peerid(&self) -> &Id {
        &self.peerid
    }

    pub fn service_peer(&self) -> Option<&PeerInfo> {
        self.service_peer.as_ref()
    }

    pub fn service_node(&self) -> Option<&NodeInfo> {
        self.service_node.as_ref()
    }

    async fn lookup(&mut self) -> Result<()> {
        info!("{} is trying to find peer {} and hosting node via DHT network...", self.app_name, self.peerid);

        let node = self.node.lock().unwrap();
        let mut peers = node.find_peer(&self.peerid, Some(4), None).await.map_err(|e| {
            Error::State(format!("{} find peer error: {}", self.app_name, e))
        })?;

        let output = format!("No peers with peerid {} is found at this moment, please try it later!!!", self.peerid);
        if peers.is_empty() {
            warn!("{}", &output);
            return Err(Error::State(output));
        }

        debug!("Discovered {} service peers, extracting each node's infomation...", peers.len());

        peers.shuffle(&mut thread_rng());
        while let Some(peer) = peers.pop() {
            let nodeid = peer.nodeid();
            debug!("{} is trying lookup service node {} ...", self.app_name, nodeid);

            let result = node.find_node(nodeid, None).await.map_err(|e| {
                warn!("Failed to find node {}, error: {}", nodeid, e);
                e
            })?;

            if result.is_empty() {
                warn!("No service node {} was found! Go on looking next node ...", nodeid);
                continue;
            }

            let mut ni = None;
            if let Some(v6) = result.v6() {
                ni = Some(v6.clone());
            }
            if let Some(v4) = result.v4() {
                ni = Some(v4.clone());
            }
            let Some(ni) = ni else {
                continue;
            };

            info!("Service peer {} and its hosting node {} were found in succeess.", peer.id(), node.id());
            self.service_peer = Some(peer);
            self.service_node = Some(ni);
            return Ok(());
        }

        warn!("{}", &output);
        Err(Error::State(output))
    }

    async fn load_internal(&mut self) -> Result<()> {
        let path = self.path.clone();
        let handle = task::spawn(async move {
            let mut buf = vec![];
            let mut fp = match File::open(&path) {
                Ok(fp) => fp,
                Err(e) => {
                    warn!("Failed to open cached file {} with error: {e}.", path.display());
                    return None;
                }
            };

            if let Err(e) = fp.read_to_end(&mut buf) {
                warn!("Failed to read conetnt from cache file with error {e}");
                return None;
            }

            let reader = cbor::Reader::new(&buf);
            let val: CVal = match ciborium::de::from_reader(reader) {
                Ok(v) => v,
                Err(e) => {
                   warn!("Failed to parse data from cached file with error: {e} -
                        cached file might be broken");
                    return None;
                }
            };

            let mut peer = None;
            let mut ni = None;
            if let Some(root) = val.as_map() {
                for (k,v) in root {
                    let Some(k) = k.as_text() else {
                        break;
                    };
                    match k {
                        "peer" => peer = PeerInfo::from_cbor(v),
                        "node" => ni = NodeInfo::from_cbor(v),
                        _ => break
                    }
                }
            }
            let Some(peer) = peer else {
                warn!("Missing peer information");
                return None;
            };

            let Some(ni) = ni else {
                warn!("Missing node information");
                return None;
            };

            Some((peer, ni))
        });

        let Ok(Some(v)) = handle.await else {
            return Ok(());
        };

        self.service_peer = Some(v.0);
        self.service_node = Some(v.1);
        Ok(())
    }

    async fn store_internal(&self) -> Result<()> {
        let path = self.path.clone();
        let Some(peer) = self.service_peer.as_ref().map(|v| v.clone()) else {
            return Ok(())
        };
        let Some(ni) = self.service_node.as_ref().map(|v| v.clone()) else {
            return Ok(())
        };

        task::spawn(async move {
            let val = CVal::Map(vec![
                (
                    CVal::Text(String::from("peer")),
                    peer.to_cbor(),
                ),
                (
                    CVal::Text(String::from("node")),
                    ni.to_cbor()
                )
            ]);

            let mut buf = vec![];
            let writer = cbor::Writer::new(&mut buf);
            if let Err(e) = ciborium::ser::into_writer(&val, writer) {
                return;
            }
            let Ok(mut fp) = File::create(path) else {
                return;
            };
            _ = fp.write_all(&buf);
        }).await
        .map_err(|e|
            Error::State("{e}".into())
        )
    }

    pub async fn load(&mut self) -> Result<()> {
        if let Err(_) = self.load_internal().await {
            self.lookup().await?;
            self.store_internal().await?;
        }
        Ok(())
    }

    pub async fn store(&self) -> Result<()> {
        unimplemented!()
    }
}


use std::fmt;
use std::rc::Rc;
use std::any::Any;
use ciborium::Value as CVal;

use crate::{
    Id,
    PeerInfo,
    NodeInfo,
    Error,
    core::{
        version,
        Result,
        peer_info::PackBuilder
    }
};

use super::{
    msg::{
        Kind, Method, Msg,
        Data as MsgData
    },
    lookup_rsp::{
        Msg as LookupResponse,
        Data as LookuResponseData
    },
};

pub(crate) struct Message {
    base_data   : MsgData,
    lookup_data : LookuResponseData,

    peers       : Vec<PeerInfo>,
}

impl Msg for Message {
    fn data(&self) -> &MsgData {
        &self.base_data
    }

    fn data_mut(&mut self) -> &mut MsgData {
        &mut self.base_data
    }

    fn from_cbor(&mut self, input: &CVal) -> Option<()> {
        let root = input.as_map()?;
        for (k,v) in root {
            let k = k.as_text()?;
            match k {
                "y" => {},
                "t" => {
                    let txid = v.as_integer()?.try_into().unwrap();
                    self.set_txid(txid);
                },
                "v" => {
                    let ver = v.as_integer()?.try_into().unwrap();
                    self.set_ver(ver);
                },
                "r" => {
                    let map = v.as_map()?;
                    for (k,v) in map {
                        let k = k.as_text()?;
                        match k {
                            "n4" => {
                                self.populate_closest_nodes4({
                                    let mut nodes = Vec::new();
                                    for item in v.as_array()?.iter() {
                                        nodes.push(Rc::new(NodeInfo::from_cbor(item)?));
                                    }
                                    nodes
                                });
                            },
                            "n6" => {
                                self.populate_closest_nodes6({
                                    let mut nodes = Vec::new();
                                    for item in v.as_array()?.iter() {
                                        nodes.push(Rc::new(NodeInfo::from_cbor(item)?));
                                    }
                                    nodes
                                });
                            },
                            "tok" => {
                                self.populate_token(
                                    v.as_integer()?.try_into().unwrap()
                                );
                            },
                            "p" => {
                                let v = v.as_array()?;
                                let mut leading_peer = true;
                                let mut peerid = None;
                                for item in v.iter() {
                                    let v = item.as_array()?;
                                    if leading_peer {
                                        peerid = Some(Id::from_cbor(v.get(0)?)?);
                                        leading_peer = false;
                                    }
                                    let nodeid = Id::from_cbor(v.get(1)?)?;
                                    let origin = match v.get(2)?.is_null() {
                                        true => None,
                                        false => Id::from_cbor(v.get(2)?)
                                    };
                                    let port = v.get(3)?.as_integer()?.try_into().unwrap();
                                    let alt = v.get(4)?.as_text();
                                    let sig = v.get(5)?.as_bytes()?;

                                    let peer = PackBuilder::new(nodeid)
                                        .with_peerid(peerid.clone())
                                        .with_origin(origin)
                                        .with_port(port)
                                        .with_url(alt.map(|v|v.to_string()))
                                        .with_sig(Some(sig.to_vec()))
                                        .build();
                                    self.peers.push(peer);
                                };
                            }
                            _ => return None
                        }
                    }
                },
                _ => return None,
            }
        }
        Some(())
    }

    fn ser(&self) -> CVal {
        let mut array = vec![];

        let mut leading_peer = true;
        self.peers.iter().for_each(|item| {
            let peer_id = if leading_peer {
                leading_peer = false;
                item.id().to_cbor()
            } else {
                CVal::Null
            };

            let nodeid  = item.nodeid().to_cbor();
            let port    = CVal::Integer(item.port().into());
            let sig     = CVal::Bytes(item.signature().to_vec());
            let origin  = match item.is_delegated() {
                true => item.origin().to_cbor(),
                false => CVal::Null
            };
            let alt_url = item.alternative_url()
                .map_or(CVal::Null, |url|CVal::Text(url.to_string()));

            let mut peer = vec![];
            peer.push(peer_id);
            peer.push(nodeid);
            peer.push(origin);
            peer.push(port);
            peer.push(alt_url);
            peer.push(sig);
            array.push(CVal::Array(peer));
        });

        let mut rsp = LookupResponse::to_cbor(self);
        if let Some(map) = rsp.as_map_mut() {
            map.push((
                CVal::Text(String::from("p")),
                CVal::Array(array))
            );
        }

        let mut root = Msg::to_cbor(self);
        if let Some(map) = root.as_map_mut() {
            map.push((
                CVal::Text(String::from("r")),
                rsp
            ));
        }
        root
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl LookupResponse for Message {
    fn data(&self) -> &LookuResponseData {
        &self.lookup_data
    }

    fn data_mut(&mut self) -> &mut LookuResponseData {
        &mut self.lookup_data
    }
}

impl Message {
    pub(crate) fn new() -> Self {
        Self {
            lookup_data: LookuResponseData::new(),
            base_data: MsgData::new(
                Kind::Response,
                Method::FindPeer,
                0
            ),
            peers: Vec::new(),
        }
    }

    pub(crate) fn peers(&self) -> &[PeerInfo] {
        self.peers.as_ref()
    }

    pub(crate) fn populate_peers(&mut self, peers: Vec<PeerInfo>) {
        self.peers = peers
    }
}

impl TryFrom<CVal> for Box<Message> {
    type Error = Error;
    fn try_from(input: CVal) -> Result<Box<Message>> {
        let mut msg = Self::new(Message::new());
        if let None =  msg.from_cbor(&input) {
                return Err(Error::Protocol(
                    format!("Invalid cobor value for find_peer_rsp message")));
        }
        Ok(msg)
    }
}

impl fmt::Display for Message {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f,
            "y:{},m:{},t:{},r: {{",
            self.kind(),
            self.method(),
            self.txid() as u32,
        )?;

        if let Some(nodes4) = self.nodes4() {
            let mut first = true;
            if !nodes4.is_empty() {
                write!(f, "n4:")?;
                for item in nodes4.iter() {
                    if !first {
                        first = false;
                        write!(f, ",")?;
                    }
                    write!(f, "[{}]", item)?;
                }
            }
        }

        if let Some(nodes6) = self.nodes6() {
            let mut first = true;
            if !nodes6.is_empty() {
                write!(f, "n6:")?;
                for item in nodes6.iter() {
                    if !first {
                        first = false;
                        write!(f, ",")?;
                    }
                    write!(f, "[{}]", item)?;
                }
            }
        }

        if self.token() != 0 {
            write!(f, ",tok:{}", self.token())?;
        }

        let mut first = true;
        if !self.peers.is_empty() {
            write!(f, ",p:")?;
            for item in self.peers.iter() {
                if !first {
                    first = false;
                    write!(f, ",")?;
                }
                write!(f, "[{}]", item)?;
            }
        }

        write!(f,
            "}},v:{}",
            version::format_version(self.ver())
        )?;
        Ok(())
    }
}

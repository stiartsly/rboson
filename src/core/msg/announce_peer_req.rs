use std::fmt;
use std::rc::Rc;
use std::any::Any;
use ciborium::Value as CVal;

use crate::{
    Id,
    PeerInfo,
    Error,
    error::Result
};

use crate::core::{
    version,
    peer_info::PackBuilder,
};

use super::{Method, Kind};
use super::msg::{
    Msg,
    Data as MsgData
};

pub(crate) struct Message {
    base_data: MsgData,

    token: i32,
    peer: Option<Rc<PeerInfo>>, // Must cantain a value.
}

impl Msg for Message {
    fn data(&self) -> &MsgData {
        &self.base_data
    }

    fn data_mut(&mut self) -> &mut MsgData {
        &mut self.base_data
    }

    fn from_cbor(&mut self, input: &CVal)-> Option<()> {
        let mut peerid = None;
        let mut nodeid = None;
        let mut port = 0u16;
        let mut alt = None;
        let mut sig = None;

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
                "q" => {
                    let map = v.as_map()?;
                    for (k,v) in map {
                        let k = k.as_text()?;
                        match k {
                            "t" => peerid = Some(Id::from_cbor(v)?),
                            "x" => nodeid = Some(Id::from_cbor(v)?),
                            "p" => port = v.as_integer()?.try_into().unwrap(),
                            "alt" => alt = Some(v.as_text()?),
                            "sig" => sig = Some(v.as_bytes()?.clone()),
                            "tok" => self.token = v.as_integer()?.try_into().unwrap(),
                            _ => return None,
                        }
                    }
                },
                _ => return None,
            }
        }

        let peer = PackBuilder::new(nodeid.unwrap())    // TODO: `Option::unwrap()` on a `None` value
            .with_peerid(peerid)
            .with_origin(None)
            .with_port(port)
            .with_url(alt.map(|v| v.to_string()))
            .with_sig(sig)
            .build();

        self.peer = Some(Rc::new(peer));
        Some(())
    }

    fn ser(&self) -> CVal {
        let mut req = vec![
            (
                CVal::Text(String::from("t")),
                self.peer().id().to_cbor(),
            ),
            (
                CVal::Text(String::from("tok")),
                CVal::Integer(self.token.into()),
            ),
            (
                CVal::Text(String::from("p")),
                CVal::Integer(self.peer().port().into()),
            ),
            (
                CVal::Text(String::from("sig")),
                CVal::Bytes(self.peer().signature().to_vec()),
            ),
            (
                CVal::Text(String::from("x")),
                self.peer().nodeid().to_cbor(),
            )
        ];

        self.peer().alternative_url().map(|url| req.push((
            CVal::Text(String::from("alt")),
            CVal::Text(url.to_string()),
        )));

        let mut root = Msg::to_cbor(self);
        root.as_map_mut().map(|map| map.push((
            CVal::Text(Kind::Request.to_key().to_string()),
            CVal::Map(req)
        )));

        root
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl Message {
    pub(crate) fn new() -> Self {
        Self {
            base_data: MsgData::new(
                Kind::Request,
                Method::AnnouncePeer,
                0
            ),
            token: 0,
            peer: None,
        }
    }

    pub(crate) fn token(&self) -> i32 {
        self.token
    }

    pub(crate) fn peer(&self) -> Rc<PeerInfo> {
        self.peer.as_ref().unwrap().clone()
    }

    pub(crate) fn with_token(&mut self, token: i32) {
        self.token = token
    }

    pub(crate) fn with_peer(&mut self, peer: Rc<PeerInfo>) {
        self.peer = Some(peer)
    }

    pub(crate) fn target(&self) -> &Id {
        self.peer.as_ref().unwrap().id()
    }
}

impl TryFrom<CVal> for Box<Message> {
    type Error = Error;
    fn try_from(input: CVal) -> Result<Box<Message>> {
        let mut msg = Self::new(Message::new());
        if let None =  msg.from_cbor(&input) {
            return Err(Error::Protocol(
                format!("Invalid cobor value for announce_peer_req message")));
        }
        Ok(msg)
    }
}

impl fmt::Display for Message {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let peer = self.peer.as_ref().unwrap();
        write!(f,
            "y:{},m:{},t:{},q: {{",
            self.kind(),
            self.method(),
            self.txid()
        )?;

        write!(f, "t:{},n:{},p:{}",
            peer.id(),
            peer.nodeid(),
            peer.port()
        )?;

        if let Some(url) = peer.alternative_url() {
            write!(f, ",alt:{}", url)?;
        }
        write!(f, ",sig:{},tok:{}",
            hex::encode(peer.signature()),
            self.token as u32
        )?;

        write!(f,
            "}},v:{}",
            version::canonical_version(self.ver())
        )?;
        Ok(())
    }
}

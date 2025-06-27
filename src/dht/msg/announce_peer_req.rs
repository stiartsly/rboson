use std::fmt;
use std::rc::Rc;
use std::any::Any;
use ciborium::Value as CVal;

use crate::{
    unwrap,
    Id,
    PeerInfo,
    Error,
    error::Result
};

use crate::core::{
    version,
    peer_info::PackBuilder,
};

use super::msg::{
    Msg, Method, Kind,
    Data as MsgData
};

pub(crate) struct Message {
    base_data: MsgData,

    token: i32,
    peerid: Option<Id>,
    origin: Option<Id>,  // Optional, only for the delegated peer
    port: Option<u16>,
    url: Option<String>,
    sig: Option<Vec<u8>>
}

impl Msg for Message {
    fn data(&self) -> &MsgData {
        &self.base_data
    }

    fn data_mut(&mut self) -> &mut MsgData {
        &mut self.base_data
    }

    fn from_cbor(&mut self, input: &CVal)-> Option<()> {
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
                            "t" => self.peerid = Some(Id::from_cbor(v)?),
                            "x" => self.origin = Id::from_cbor(v),
                            "p" => self.port = Some(v.as_integer()?.try_into().unwrap()),
                            "alt" => self.url = v.as_text().map(|v|v.to_string()),
                            "sig" => self.sig = Some(v.as_bytes()?.clone()),
                            "tok" => self.token = v.as_integer()?.try_into().unwrap(),
                            _ => return None,
                        }
                    }
                },
                _ => return None,
            }
        }

        if self.peerid.is_none() || self.port.is_none() || self.sig.is_none() {
            return None;
        }
        Some(())
    }

    fn ser(&self) -> CVal {
        let mut req = vec![
            (
                CVal::Text(String::from("t")),
                unwrap!(self.peerid).to_cbor(),
            ),
            (
                CVal::Text(String::from("tok")),
                CVal::Integer(self.token.into()),
            ),
            (
                CVal::Text(String::from("p")),
                CVal::Integer(self.port.unwrap().into())
            ),
            (
                CVal::Text(String::from("sig")),
                CVal::Bytes(unwrap!(self.sig).to_vec()),
            )
        ];

        if let Some(origin) = self.origin.as_ref() {
            req.push((
                CVal::Text(String::from("x")),
                origin.to_cbor(),
            ))
        }

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
            token:  0,
            peerid: None,
            origin: None,
            port:   None,
            url:    None,
            sig:    None,
        }
    }

    pub(crate) fn token(&self) -> i32 {
        self.token
    }

    pub(crate) fn peer(&self) -> Rc<PeerInfo> {
        Rc::new(PackBuilder::new(self.id().clone())
            .with_peerid(self.peerid.as_ref().map(|v|v.clone()))
            .with_origin(self.origin.as_ref().map(|v|v.clone()))
            .with_port(self.port.unwrap())
            .with_url(self.url.as_ref().map(|v|v.to_string()))
            .with_sig(self.sig.as_ref().map(|v|v.to_vec()))
            .build())
    }

    pub(crate) fn with_token(&mut self, token: i32) {
        self.token = token
    }

    pub(crate) fn with_peer(&mut self, peer: Rc<PeerInfo>) {
        self.peerid = Some(peer.id().clone());
        self.origin = match peer.is_delegated() {
            true => Some(peer.origin().clone()),
            false => None,
        };
        self.port = Some(peer.port());
        self.url = peer.alternative_url().map(|v|v.to_string());
        self.sig = Some(peer.signature().to_vec());
    }

    pub(crate) fn target(&self) -> &Id {
        unwrap!(self.peerid)
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
        write!(f,
            "y:{},m:{},t:{},q: {{",
            self.kind(),
            self.method(),
            self.txid()
        )?;

        write!(f, "t:{},n:{},p:{}",
            unwrap!(self.peerid),
            self.id(),
            unwrap!(self.port)
        )?;

        if let Some(url) = self.url.as_ref() {
            write!(f, ",alt:{}", url)?;
        }

        write!(f, ",sig:{},tok:{}",
            hex::encode(unwrap!(self.sig)),
            self.token as u32
        )?;

        write!(f,
            "}},v:{}",
            version::canonical_version(self.ver())
        )?;
        Ok(())
    }
}

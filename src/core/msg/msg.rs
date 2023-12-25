use std::fmt;
use std::fmt::Display;
use std::any::Any;
use std::rc::Rc;
use std::cell::RefCell;
use std::net::SocketAddr;
use ciborium;
use ciborium::Value as CVal;

use crate::{
    unwrap,
    Id,
    rpccall::RpcCall
};

#[derive(PartialEq, Clone, Copy)]
pub(crate) enum Kind {
    Error = 0,
    Request = 0x20,
    Response = 0x40,
}

impl Kind {
    const MASK: i32 = 0xE0;
    pub(crate) fn from(_type: i32) -> Kind {
        let kind = _type & Self::MASK;
        match kind {
            0x00 => Kind::Error,
            0x20 => Kind::Request,
            0x40 => Kind::Response,
            _ => panic!("invalid msg kind: {}", kind)
        }
    }

    pub(crate) fn is_valid(_type: i32) -> bool {
        match _type & Self::MASK {
            0x00 => true,
            0x20 => true,
            0x40 => true,
            _ => false,
        }
    }

    pub(crate) fn to_key(&self) -> &'static str {
        match self {
            Kind::Error => "e",
            Kind::Request => "q",
            Kind::Response => "r",
        }
    }
}

impl fmt::Display for Kind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let str = match self {
            Kind::Error => "e",
            Kind::Request => "q",
            Kind::Response => "r",
        };
        write!(f, "{}", str)?;
        Ok(())
    }
}

#[derive(PartialEq, Clone, Copy)]
pub(crate) enum Method {
    Unknown = 0x00,
    Ping = 0x01,
    FindNode = 0x02,
    AnnouncePeer = 0x03,
    FindPeer = 0x04,
    StoreValue = 0x05,
    FindValue = 0x6,
}

impl Method {
    const MASK: i32 = 0x1F;
    pub(crate) fn from(_type: i32) -> Self {
        let method = _type & Self::MASK;
        match _type & Self::MASK {
            0x00 => Method::Unknown,
            0x01 => Method::Ping,
            0x02 => Method::FindNode,
            0x03 => Method::AnnouncePeer,
            0x04 => Method::FindPeer,
            0x05 => Method::StoreValue,
            0x06 => Method::FindValue,
            _ => panic!("invalid msg method: {}", method)
        }
    }

    pub(crate) fn is_valid(_type: i32) -> bool {
        let method = _type & Self::MASK;
        method >= 0 && method <= 0x06
    }
}

impl fmt::Display for Method {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let str = match self {
            Method::Unknown => "unknown",
            Method::Ping => "ping",
            Method::FindNode => "find_node",
            Method::AnnouncePeer => "announce_peer",
            Method::FindPeer => "find_peer",
            Method::StoreValue => "store_value",
            Method::FindValue => "find_value",
        };
        write!(f, "{}", str)?;
        Ok(())
    }
}

pub(crate) struct Data {
    id: Option<Id>,
    remote_id: Option<Id>,

    origin : Option<SocketAddr>,
    remote : Option<SocketAddr>,

    associated_call: Option<Rc<RefCell<RpcCall>>>,

    type_  : i32,
    txid   : i32,
    ver    : i32,
}

impl Data {
   pub(crate) fn new(kind: Kind, method: Method, txid: i32) -> Self {
        Self {
            id: None,
            remote_id: None,
            origin: None,
            remote: None,
            associated_call: None,
            type_: kind as i32 | method as i32,
            txid: txid,
            ver: 0
        }
    }
}

pub(crate) trait Msg: Display {
    fn data(&self) -> &Data;
    fn data_mut(&mut self) -> &mut Data;

    fn _type(&self) -> i32 {
        self.data().type_
    }

    fn kind(&self) -> Kind {
        Kind::from(self.data().type_)
    }

    fn method(&self) -> Method {
        Method::from(self.data().type_)
    }

    fn id(&self) -> &Id {
        unwrap!(self.data().id)
    }

    fn remote_id(&self) -> &Id {
        unwrap!(self.data().remote_id)
    }

    fn origin(&self) -> &SocketAddr {
        unwrap!(self.data().origin)
    }

    fn remote_addr(&self) -> &SocketAddr {
        unwrap!(self.data().remote)
    }

    fn txid(&self) -> i32 {
        self.data().txid
    }

    fn ver(&self) -> i32 {
        self.data().ver
    }

    fn associated_call(&self) -> Option<Rc<RefCell<RpcCall>>> {
        self.data().associated_call.as_ref().map(|v|v.clone())
    }

    fn set_id(&mut self, id: &Id) {
        self.data_mut().id = Some(id.clone())
    }

    fn set_origin(&mut self, addr: &SocketAddr) {
        self.data_mut().origin = Some(addr.clone())
    }

    fn set_remote(&mut self, id: &Id, addr: &SocketAddr) {
        self.data_mut().remote_id = Some(id.clone());
        self.data_mut().remote = Some(addr.clone())
    }

    fn set_txid(&mut self, txid: i32) {
        self.data_mut().txid = txid
    }

    fn set_ver(&mut self, ver: i32) {
        self.data_mut().ver = ver
    }

    fn with_associated_call(&mut self, call: Rc<RefCell<RpcCall>>) {
        self.data_mut().associated_call = Some(call)
    }

    fn to_cbor(&self) -> CVal {
        CVal::Map(vec![
            (
                CVal::Text(String::from("y")),
                CVal::Integer(self._type().into())
            ),
            (
                CVal::Text(String::from("t")),
                CVal::Integer(self.txid().into())
            ),
            (
                CVal::Text(String::from("v")),
                CVal::Integer(self.ver().into())
            )
        ])
    }

    fn from_cbor(&mut self, _: &CVal) -> Option<()>;
    fn as_any(&self) -> &dyn Any;
    fn ser(&self) -> CVal;
}

impl From<&Box<dyn Msg>> for CVal {
    fn from(msg: &Box<dyn Msg>) -> Self {
        msg.ser()
    }
}

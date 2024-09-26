// CBOR and Message handling
pub(crate) mod cbor;
pub(crate) mod msg;

pub(crate) mod error_msg;

pub(crate) mod lookup_req;
pub(crate) mod lookup_rsp;

pub(crate) mod ping_req;
pub(crate) mod ping_rsp;

pub(crate) mod find_node_req;
pub(crate) mod find_node_rsp;

pub(crate) mod find_peer_req;
pub(crate) mod find_peer_rsp;
pub(crate) mod announce_peer_req;
pub(crate) mod announce_peer_rsp;

pub(crate) mod find_value_req;
pub(crate) mod find_value_rsp;
pub(crate) mod store_value_req;
pub(crate) mod store_value_rsp;

use std::rc::Rc;
use std::cell::RefCell;
use ciborium;
use ciborium::Value as CVal;

use cbor::{Reader, Writer};
use msg::{Msg, Kind, Method};
use crate::core::error::{
    Error,
    Result
};

pub(crate) fn deser(buf: &[u8]) -> Result<Rc<RefCell<Box<dyn Msg>>>> {
    let reader = Reader::new(buf);
    let value: CVal = ciborium::de::from_reader(reader)
        .map_err(|e|
            return Error::Protocol(format!("Reading cobor error: {}", e))
        )?;

    let mtype = {
        let map = value.as_map().unwrap();
        let mut _v = None;
        for (k,v) in map.iter() {
            let k = k.as_text().unwrap();
            if k == "y" {
                _v = Some(v);
                break;
            }
        }
        _v.unwrap().as_integer()
            .unwrap()
            .try_into()
            .unwrap()
    };
    if !Kind::is_valid(mtype) || !Method::is_valid(mtype) {
        return Err(Error::Protocol(format!(
            "Invalid message kind {} or method {}", Kind::from(mtype), Method::from(mtype)
        )));
    }

    let msg = match Kind::from(mtype) {
        Kind::Error     => value.try_into().map(|v: Box<error_msg::Message>| v as Box<dyn Msg>)?,
        Kind::Request   => match Method::from(mtype) {
            Method::Ping        => value.try_into().map(|v: Box<ping_req::Message>| v as Box<dyn Msg>)?,
            Method::FindNode    => value.try_into().map(|v: Box<find_node_req::Message>| v as Box<dyn Msg>)?,
            Method::AnnouncePeer=> value.try_into().map(|v: Box<announce_peer_req::Message>| v as Box<dyn Msg>)?,
            Method::FindPeer    => value.try_into().map(|v: Box<find_peer_req::Message>| v as Box<dyn Msg>)?,
            Method::StoreValue  => value.try_into().map(|v: Box<store_value_req::Message>| v as Box<dyn Msg>)?,
            Method::FindValue   => value.try_into().map(|v: Box<find_value_req::Message>| v as Box<dyn Msg>)?,
            Method::Unknown     => return Err(Error::Protocol(format!(
                "Invalid request message: {}, ignored it", Method::from(mtype)
            )))
        },
        Kind::Response  => match Method::from(mtype) {
            Method::Ping        => value.try_into().map(|v: Box<ping_rsp::Message>| v as Box<dyn Msg>)?,
            Method::FindNode    => value.try_into().map(|v: Box<find_node_rsp::Message>| v as Box<dyn Msg>)?,
            Method::AnnouncePeer=> value.try_into().map(|v: Box<announce_peer_rsp::Message>| v as Box<dyn Msg>)?,
            Method::FindPeer    => value.try_into().map(|v: Box<find_peer_rsp::Message>| v as Box<dyn Msg>)?,
            Method::StoreValue  => value.try_into().map(|v: Box<store_value_rsp::Message>| v as Box<dyn Msg>)?,
            Method::FindValue   => value.try_into().map(|v: Box<find_value_rsp::Message>| v as Box<dyn Msg>)?,
            Method::Unknown     => return Err(Error::Protocol(format!(
                "Invalid response message: {}, ignored it", Method::from(mtype)
            )))
        }
    };
    Ok(Rc::new(RefCell::new(msg)))
}

pub(crate) fn serialize(msg: Rc<RefCell<Box<dyn Msg>>>) -> Vec<u8> {
    let mut val: CVal = msg.borrow().ser();
    let mut buf = Vec::with_capacity(1024);

    ciborium::ser::into_writer(
        &mut val,
        Writer::new(&mut buf)
    ).unwrap();

    buf
}

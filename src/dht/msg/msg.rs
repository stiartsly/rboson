use std::{
    fmt,
    rc::Rc,
    cell::RefCell,
    net::SocketAddr,
    result::Result as StdResult,
    sync::atomic::{AtomicU32, Ordering}
};
use serde_cbor::value::{Value as CborValue, from_value};
use serde::{Deserialize, Serialize};
use rand::RngExt;

use crate::{
    utils,
    Id,
    Value,
    NodeInfo,
    PeerInfo,
    errors::{Error, Result, ProtocolError},
    core::version,
    dht::rpc::RpcCall,
    dht::msg::{
        ErrorBody,
        FindNodeRequest,
        FindNodeResponse,
        FindPeerRequest,
        FindPeerResponse,
        FindValueRequest,
        FindValueResponse,
        AnnouncePeerRequest,
        StoreValueRequest,
    },
};

#[derive(Clone, Copy, PartialEq)]
#[repr(u8)]
pub(crate) enum Kind {
    Error = 0,
    Request = 0x20,
    Response = 0x40,
}

impl Kind {
    const MASK: i32 = 0xE0;
}

impl TryFrom<i32> for Kind {
    type Error = Error;
    fn try_from(_type: i32) -> Result<Self> {
        let kind = _type & Self::MASK;
        match kind {
            0x00 => Ok(Kind::Error),
            0x20 => Ok(Kind::Request),
            0x40 => Ok(Kind::Response),
            _ => Err(ProtocolError::new(format!("invalid msg kind: {}", kind)))
        }
    }
}

impl fmt::Display for Kind {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(match *self {
            Kind::Error => "e",
            Kind::Request => "q",
            Kind::Response => "r",
        })
    }
}

#[derive(Clone, Copy, PartialEq)]
#[repr(u8)]
pub(crate) enum Method {
    Unknown     = 0x00,
    Ping        = 0x01,
    FindNode    = 0x02,
    AnnouncePeer= 0x03,
    FindPeer    = 0x04,
    StoreValue  = 0x05,
    FindValue   = 0x06,
}

impl Method {
    const MASK: i32 = 0x1F;
}

impl TryFrom<i32> for Method {
    type Error = Error;
    fn try_from(_type: i32) -> Result<Self> {
        let method = _type & Self::MASK;
        match method {
            0x00 => Ok(Method::Unknown),
            0x01 => Ok(Method::Ping),
            0x02 => Ok(Method::FindNode),
            0x03 => Ok(Method::AnnouncePeer),
            0x04 => Ok(Method::FindPeer),
            0x05 => Ok(Method::StoreValue),
            0x06 => Ok(Method::FindValue),
            _ => Err(ProtocolError::new(format!("invalid msg method: {}", method)))
        }
    }
}

impl fmt::Display for Method {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(match *self  {
            Method::Unknown => "unknown",
            Method::Ping => "ping",
            Method::FindNode => "find_node",
            Method::AnnouncePeer => "announce_peer",
            Method::FindPeer => "find_peer",
            Method::StoreValue => "store_value",
            Method::FindValue => "find_value",
        })
    }
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub(crate) enum Body {
    FindNodeRequest(FindNodeRequest),
    FindNodeResponse(FindNodeResponse),
    FindPeerRequest(FindPeerRequest),
    FindPeerResponse(FindPeerResponse),
    FindValueRequest(FindValueRequest),
    FindValueResponse(FindValueResponse),
    AnnouncePeerRequest(AnnouncePeerRequest),
    StoreValueRequest(StoreValueRequest),
    Error(ErrorBody),
}

impl Body {
    fn from_err(value: CborValue) -> Result<Option<Self>> {
        let err_cb = |e| ProtocolError::new(format!("Decoding error body failed: {}", e));
        Ok(from_value::<ErrorBody>(value)
            .map(Body::Error)
            .map(Some)
            .map_err(err_cb)?)
    }

    fn from_req(method: Method, value: CborValue) -> Result<Option<Self>> {
        let err_cb = |e| ProtocolError::new(format!("Decoding {} request error: {}", method, e));
        Ok(match method {
            Method::Ping => None,
            Method::FindNode => from_value::<FindNodeRequest>(value)
                .map(Body::FindNodeRequest)
                .map(Some)
                .map_err(err_cb)?,
            Method::AnnouncePeer => from_value::<AnnouncePeerRequest>(value)
                .map(Body::AnnouncePeerRequest)
                .map(Some)
                .map_err(err_cb)?,
            Method::FindPeer => from_value::<FindPeerRequest>(value)
                .map(Body::FindPeerRequest)
                .map(Some)
                .map_err(err_cb)?,
            Method::StoreValue => from_value::<StoreValueRequest>(value)
                .map(Body::StoreValueRequest)
                .map(Some)
                .map_err(err_cb)?,
            Method::FindValue => from_value::<FindValueRequest>(value)
                .map(Body::FindValueRequest)
                .map(Some)
                .map_err(err_cb)?,
            Method::Unknown => return Err(ProtocolError::new("invalid unknown request".to_string())),
        })
    }

    fn from_rsp(method: Method, value: CborValue) -> Result<Option<Self>> {
        let err_cb = |e| ProtocolError::new(format!("Decoding {} response error: {}", method, e));
        Ok(match method {
            Method::Ping | Method::AnnouncePeer | Method::StoreValue => None,
            Method::FindNode => from_value::<FindNodeResponse>(value)
                .map(Body::FindNodeResponse)
                .map(Some)
                .map_err(err_cb)?,
            Method::FindPeer => from_value::<FindPeerResponse>(value)
                .map(Body::FindPeerResponse)
                .map(Some)
                .map_err(err_cb)?,
            Method::FindValue => from_value::<FindValueResponse>(value)
                .map(Body::FindValueResponse)
                .map(Some)
                .map_err(err_cb)?,
            Method::Unknown => return Err(ProtocolError::new("invalid unknown response".to_string())),
        })
    }
}

impl fmt::Display for Body {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Body::FindNodeRequest(body)   => write!(f, "{}", body),
            Body::FindNodeResponse(body)  => write!(f, "{}", body),
            Body::FindPeerRequest(body)   => write!(f, "{}", body),
            Body::FindPeerResponse(body)  => write!(f, "{}", body),
            Body::FindValueRequest(body)  => write!(f, "{}", body),
            Body::FindValueResponse(body) => write!(f, "{}", body),
            Body::AnnouncePeerRequest(body) => write!(f, "{}", body),
            Body::StoreValueRequest(body) => write!(f, "{}", body),
            Body::Error(body)             => write!(f, "{}", body),
        }
    }
}

pub(crate) type TxId = u32;
static NEXT_TXID: AtomicU32 = AtomicU32::new(0);

fn next_txid_after(current: u32, step: u32) -> u32 {
    let next = current.wrapping_add(step);
    if next == 0 { step } else { next }
}

fn next_txid() -> TxId {
    let step = rand::rng().random_range(1..512);
    let current = NEXT_TXID.fetch_update(Ordering::Relaxed, Ordering::Relaxed, |current| {
        Some(next_txid_after(current, step))
    }).expect("transaction ID update must succeed");

    next_txid_after(current, step)
}

#[derive(Clone)]
#[derive(Deserialize)]
#[serde(try_from = "SerdeCborMessage")]
pub(crate) struct Message {
    nodeid  : Option<Id>,        // The DHT node Id of the message sender.

    kind    : Kind,
    method  : Method,
    txid    : TxId,
    ver     : i32,

    body    : Option<Body>,

    associated_call : Option<Rc<RefCell<RpcCall>>>,
    remote_addr     : Option<SocketAddr>,
    remote_id       : Option<Id>,
}

impl Message {
    pub(crate) const MIN_BYTES: usize = 10;

    fn new(kind: Kind, method: Method, txid: u32, body: Option<Body>) -> Self {
        Self {
            nodeid: None,
            kind,
            method,
            txid,
            ver: version::ver(),
            body,
            associated_call: None,
            remote_addr: None,
            remote_id: None,
        }
    }

    fn composite_type(&self) -> i32 {
        (self.kind as i32) | (self.method as i32)
    }

    pub(crate) fn kind(&self) -> Kind {
        self.kind
    }

    pub(crate) fn method(&self) -> Method {
        self.method
    }

    pub(crate) fn is_req(&self) -> bool {
        self.kind == Kind::Request
    }

    #[allow(unused)]
    pub(crate) fn is_rsp(&self) -> bool {
        self.kind == Kind::Response
    }

    #[allow(unused)]
    pub(crate) fn is_err(&self) -> bool {
        self.kind == Kind::Error
    }

    pub(crate) fn nodeid(&self) -> &Id {
        self.nodeid.as_ref().expect("Id not set")
    }

    pub(crate) fn set_nodeid(&mut self, id: Id) {
        self.nodeid = Some(id)
    }

    pub(crate) fn txid(&self) -> TxId {
        self.txid
    }

    pub(crate) fn body(&self) -> Option<&Body> {
        self.body.as_ref()
    }

    pub(crate) fn ver(&self) -> i32 {
        self.ver
    }

    #[allow(unused)]
    pub(crate) fn readable_version(&self) -> String {
        version::format_version(self.ver)
    }

    pub(crate) fn associated_call(&self) -> Option<Rc<RefCell<RpcCall>>> {
        self.associated_call.clone()
    }

    pub(crate) fn set_associated_call(&mut self, call: Rc<RefCell<RpcCall>>) {
        self.associated_call = Some(call);
    }

    pub(crate) fn remote_id(&self) -> &Id {
        self.remote_id.as_ref().expect("remote ID not set")
    }

    pub(crate) fn remote_addr(&self) -> &SocketAddr {
        self.remote_addr.as_ref().expect("remote address not set")
    }

    pub(crate) fn set_remote(&mut self, id: Id, addr: SocketAddr) -> &mut Self {
        self.remote_id = Some(id);
        self.remote_addr = Some(addr);
        self
    }
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct SerdeCborMessage {
    #[serde(rename = "y")]
    type_: i32,
    #[serde(rename = "t")]
    txid: u32,
    #[serde(rename = "v")]
    ver: i32,

    #[serde(rename = "q")]
    #[serde(skip_serializing_if = "utils::is_default")]
    req: Option<CborValue>,

    #[serde(rename = "r")]
    #[serde(skip_serializing_if = "utils::is_default")]
    rsp: Option<CborValue>,

    #[serde(rename = "e")]
    #[serde(skip_serializing_if = "utils::is_default")]
    err: Option<CborValue>,
}

#[derive(Serialize)]
struct SerdeJsonMessage<'a> {
    #[serde(rename = "y")]
    type_: i32,
    #[serde(rename = "t")]
    txid: u32,
    #[serde(rename = "v", serialize_with = "utils::serialize_ver")]
    ver: i32,

    #[serde(rename = "q", skip_serializing_if = "utils::is_default")]
    req: Option<&'a Body>,
    #[serde(rename = "r", skip_serializing_if = "utils::is_default")]
    rsp: Option<&'a Body>,
    #[serde(rename = "e", skip_serializing_if = "utils::is_default")]
    err: Option<&'a Body>,
}

impl Serialize for Message {
    fn serialize<S>(&self, se: S) -> StdResult<S::Ok, S::Error>
    where S: serde::Serializer,
    {
        if !se.is_human_readable() {
            let message = SerdeCborMessage::try_from(self)
                .map_err(serde::ser::Error::custom)?;
            return message.serialize(se);
        }

        let body = self.body();
        SerdeJsonMessage {
            type_: self.composite_type(),
            txid: self.txid,
            ver: self.ver,
            req: (self.kind == Kind::Request).then_some(body).flatten(),
            rsp: (self.kind == Kind::Response).then_some(body).flatten(),
            err: (self.kind == Kind::Error).then_some(body).flatten(),
        }.serialize(se)
    }
}

impl TryFrom<&Message> for SerdeCborMessage {
    type Error = serde_cbor::Error;

    fn try_from(msg: &Message) -> StdResult<Self, Self::Error> {
        let type_ = msg.composite_type();
        let txid = msg.txid;
        let ver  = msg.ver;
        let body = msg.body();

        let req = if msg.kind() == Kind::Request {
            body.map(serde_cbor::value::to_value).transpose()?
        } else {
            None
        };
        let rsp = if msg.kind() == Kind::Response {
            body.map(serde_cbor::value::to_value).transpose()?
        } else {
            None
        };
        let err = if msg.kind() == Kind::Error {
            body.map(serde_cbor::value::to_value).transpose()?
        } else {
            None
        };

        Ok(Self { type_, txid, ver, req, rsp, err })
    }
}

impl TryFrom<SerdeCborMessage> for Message {
    type Error = Error;

    fn try_from(s: SerdeCborMessage) -> Result<Self> {
        let kind: Kind = s.type_.try_into()?;
        let method: Method = s.type_.try_into()?;

        let err =  if kind == Kind::Error {
            s.err.map(Body::from_err).transpose()?.flatten()
        } else {
            None
        };
        let req = if kind == Kind::Request {
            s.req.map(|v| Body::from_req(method, v)).transpose()?.flatten()
        } else {
            None
        };
        let rsp = if kind == Kind::Response {
            s.rsp.map(|v| Body::from_rsp(method, v)).transpose()?.flatten()
        } else {
            None
        };
        let body = match s.type_ & Kind::MASK {
            0x00 => err,
            0x20 => req,
            0x40 => rsp,
            _ => None,
        };

        let mut msg = Message::new(kind, method, s.txid, body);
        msg.ver = s.ver;
        Ok(msg)
    }
}

impl AsRef<Message> for Message {
    fn as_ref(&self) -> &Message {
        self
    }
}

#[inline]
fn request(method: Method, body: Option<Body>) -> Message {
    Message::new(Kind::Request, method, next_txid(), body)
}

#[inline]
fn response(method: Method, txid: TxId, body: Option<Body>) -> Message {
    Message::new(Kind::Response, method, txid, body)
}

pub(crate) fn ping_request() -> Message {
    request(Method::Ping, None)
}

pub(crate) fn ping_response(txid: TxId) -> Message {
    response(Method::Ping, txid, None)
}

pub(crate) fn find_node_request(target: Id, want4: bool, want6: bool, want_token: bool) -> Message {
    let body = Body::FindNodeRequest(
        FindNodeRequest::new(target, want4, want6, want_token)
    );
    request(Method::FindNode, Some(body))
}

pub(crate) fn find_node_response(txid: TxId, nodes4: Option<Vec<NodeInfo>>, nodes6: Option<Vec<NodeInfo>>, token: i32)-> Message {
    let body = Body::FindNodeResponse(
        FindNodeResponse::new(nodes4, nodes6, token)
    );
    response(Method::FindNode, txid, Some(body))
}

pub(crate) fn find_peer_request(target: Id, want4: bool, want6: bool, expected_seq: i32, expected_count: i32) -> Message {
    let body = Body::FindPeerRequest(
        FindPeerRequest::new(target, want4, want6, expected_seq, expected_count)
    );
    request(Method::FindPeer, Some(body))
}

pub(crate) fn find_peer_response_with_nodes(txid: TxId, nodes4: Option<Vec<NodeInfo>>, nodes6: Option<Vec<NodeInfo>>) -> Message {
    let body = Body::FindPeerResponse(
        FindPeerResponse::with_nodes(nodes4, nodes6)
    );
    response(Method::FindPeer, txid, Some(body))
}

pub(crate) fn find_peer_response(txid: TxId, peers: Vec<PeerInfo>) -> Message {
    let body = Body::FindPeerResponse(
        FindPeerResponse::with_peers(peers)
    );
    response(Method::FindPeer, txid, Some(body))
}

pub(crate) fn find_value_request(target: Id, want4: bool, want6: bool, expected_seq: i32) -> Message {
    let body = Body::FindValueRequest(
        FindValueRequest::new(target, want4, want6, expected_seq)
    );
    request(Method::FindValue, Some(body))
}

pub(crate) fn find_value_response_with_nodes(txid: TxId, nodes4: Option<Vec<NodeInfo>>, nodes6: Option<Vec<NodeInfo>>)-> Message {
    let body = Body::FindValueResponse(
        FindValueResponse::with_nodes(nodes4, nodes6)
    );
    response(Method::FindValue, txid, Some(body))
}

pub(crate) fn find_value_response(txid: TxId, value: Value) -> Message {
    let body = Body::FindValueResponse(
        FindValueResponse::with_value(value)
    );
    response(Method::FindValue, txid, Some(body))
}

pub(crate) fn store_value_request(value: Value, token: i32, expected_seq: i32) -> Message {
    let body = Body::StoreValueRequest(
        StoreValueRequest::new(value, token, expected_seq)
    );
    request(Method::StoreValue, Some(body))
}

pub(crate) fn store_value_response(txid: TxId) -> Message {
    response(Method::StoreValue, txid, None)
}

pub(crate) fn announce_peer_request(peer: PeerInfo, token: i32, expected_seq: i32) -> Message {
    let body = Body::AnnouncePeerRequest(
        AnnouncePeerRequest::new(peer, token, Some(expected_seq))
    );
    request(Method::AnnouncePeer, Some(body))
}

pub(crate) fn announce_peer_response(txid: TxId) -> Message {
    response(Method::AnnouncePeer, txid, None)
}

pub(crate) fn error_msg(method: Method, txid: TxId, code: i32, description: String) -> Message {
    let body = Body::Error(
        ErrorBody::new(code, description)
    );
    Message::new(Kind::Error, method, txid, Some(body))
}

impl fmt::Display for Message {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let json = serde_json::to_value(&self)
            .map_err(|_| fmt::Error)?;
        write!(f, "{}", json)
    }
}

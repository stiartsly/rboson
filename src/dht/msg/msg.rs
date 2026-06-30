use std::{
    fmt,
    rc::Rc,
    cell::RefCell,
    net::SocketAddr,
    sync::atomic::{AtomicI32, Ordering}
};
use serde_cbor::value::{Value as CborValue, from_value};
use serde::{Deserialize, Serialize};

use crate::{
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
    pub(crate) fn is_valid(_type: i32) -> bool {
        matches!(_type & Self::MASK, 0x00 | 0x20 | 0x40)
    }
}

impl From<i32> for Kind {
    fn from(_type: i32) -> Kind {
        let kind = _type & Self::MASK;
        match kind {
            0x00 => Kind::Error,
            0x20 => Kind::Request,
            0x40 => Kind::Response,
            _ => panic!("invalid msg kind: {}", kind)
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
    pub(crate) fn is_valid(_type: i32) -> bool {
        (_type & Self::MASK) <= 0x06
    }
}

impl From<i32> for Method {
    fn from(_type: i32) -> Self {
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

static NEXT_TXID: AtomicI32 = AtomicI32::new(0);
fn next_txid() -> i32 {
    let id = NEXT_TXID.fetch_add(1, Ordering::Relaxed).wrapping_add(1);
    if id == 0 {
        NEXT_TXID.fetch_add(1, Ordering::Relaxed).wrapping_add(1)
    } else {
        id
    }
}

#[derive(Clone)]
#[derive(Serialize, Deserialize)]
#[serde(into = "SerdeMessage", try_from = "SerdeMessage")]
pub(crate) struct Message {
    nodeid  : Option<Id>,        // The DHT node Id of the message sender.

    kind    : Kind,
    method  : Method,
    txid    : i32,
    ver     : i32,

    body    : Option<Body>,

    associated_call : Option<Rc<RefCell<RpcCall>>>,
    remote_addr     : Option<SocketAddr>,
    remote_id       : Option<Id>,
}

impl Message {
    pub(crate) const MIN_BYTES: usize = 10;

    fn new(kind: Kind, method: Method,  txid: i32, body: Option<Body>) -> Self {
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

    pub(crate) fn is_err(&self) -> bool {
        self.kind == Kind::Error
    }

    pub(crate) fn nodeid(&self) -> &Id {
        self.nodeid.as_ref().expect("Id not set")
    }

    pub(crate) fn set_nodeid(&mut self, id: Id) {
        self.nodeid = Some(id)
    }

    pub(crate) fn txid(&self) -> i32 {
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
struct SerdeMessage {
    #[serde(rename = "y")]
    type_: i32,
    #[serde(rename = "t")]
    txid: i32,
    #[serde(rename = "v")]
    ver: i32,

    #[serde(rename = "q")]
    #[serde(skip_serializing_if = "crate::is_default")]
    req: Option<CborValue>,

    #[serde(rename = "r")]
    #[serde(skip_serializing_if = "crate::is_default")]
    rsp: Option<CborValue>,

    #[serde(rename = "e")]
    #[serde(skip_serializing_if = "crate::is_default")]
    err: Option<CborValue>,
}

impl Into<SerdeMessage> for Message {
    fn into(self) -> SerdeMessage {
        let type_ = self.composite_type();
        let txid = self.txid;
        let ver = self.ver;
        let body = self.body();

        let req = if self.kind() == Kind::Request {
            body.and_then(|v| serde_cbor::value::to_value(v).ok())
        } else {
            None
        };
        let rsp = if self.kind() == Kind::Response {
            body.and_then(|v| serde_cbor::value::to_value(v).ok())
        } else {
            None
        };
        let err = if self.kind() == Kind::Error {
            body.and_then(|v| serde_cbor::value::to_value(v).ok())
        } else {
            None
        };

        SerdeMessage {
            type_,
            txid,
            ver,
            req,
            rsp,
            err
        }
    }
}

impl TryFrom<SerdeMessage> for Message {
    type Error = Error;
    fn try_from(s: SerdeMessage) -> Result<Self> {
        let type_ = s.type_;
        if !Kind::is_valid(type_) {
            return Err(ProtocolError::new(
                format!("Invalid message kind: {}", type_ & Kind::MASK)));
        }
        if !Method::is_valid(type_) {
            return Err(ProtocolError::new(
                format!("Invalid message method: {}", type_ & Method::MASK)));
        }

        let kind  = Kind::from(type_);
        let method = Method::from(type_);

        let err =  if kind == Kind::Error {
            s.err.and_then(|v| Body::from_err(v).ok().flatten())
        } else {
            None
        };
        let req = if kind == Kind::Request {
            s.req.and_then(|v| Body::from_req(method, v).ok().flatten())
        } else {
            None
        };
        let rsp = if kind == Kind::Response {
            s.rsp.and_then(|v| Body::from_rsp(method, v).ok().flatten())
        } else {
            None
        };
        let body = match type_ & Kind::MASK {
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

pub(crate) fn ping_request() -> Message {
    Message::new( Kind::Request, Method::Ping, next_txid(), None)
}

pub(crate) fn ping_response(txid: i32) -> Message {
    Message::new( Kind::Response, Method::Ping, txid, None)
}

pub(crate) fn find_node_request(target: Id, want4: bool, want6: bool, want_token: Option<bool>) -> Message {
    let body = Body::FindNodeRequest(
        FindNodeRequest::new(target, want4, want6, want_token.unwrap_or(false))
    );
    Message::new(Kind::Request, Method::FindNode, next_txid(), Some(body))
}

pub(crate) fn find_node_response(txid: i32, nodes4: Option<Vec<NodeInfo>>, nodes6: Option<Vec<NodeInfo>>, token: i32)-> Message {
    let body = Body::FindNodeResponse(
        FindNodeResponse::new(nodes4, nodes6, token)
    );
    Message::new(Kind::Response, Method::FindNode, txid, Some(body))
}

pub(crate) fn find_peer_request(target: Id, want4: bool, want6: bool, expected_seq: i32, expected_count: i32) -> Message {
    let body = Body::FindPeerRequest(
        FindPeerRequest::new(target, want4, want6, expected_seq, expected_count)
    );
    Message::new(Kind::Request, Method::FindPeer, next_txid(), Some(body))
}

pub(crate) fn find_peer_response_with_nodes(txid: i32, nodes4: Option<Vec<NodeInfo>>, nodes6: Option<Vec<NodeInfo>>) -> Message {
    let body = Body::FindPeerResponse(
        FindPeerResponse::with_nodes(nodes4, nodes6)
    );
    Message::new(Kind::Response, Method::FindPeer, txid, Some(body))
}

pub(crate) fn find_peer_response(txid: i32, peers: Vec<PeerInfo>) -> Message {
    let body = Body::FindPeerResponse(
        FindPeerResponse::with_peers(peers)
    );
    Message::new(Kind::Response, Method::FindPeer, txid, Some(body))
}

pub(crate) fn find_value_request(target: Id, want4: bool, want6: bool, expected_seq: i32) -> Message {
    let body = Body::FindValueRequest(
        FindValueRequest::new(target, want4, want6, expected_seq)
    );
    Message::new(Kind::Request, Method::FindValue, next_txid(),Some(body))
}

pub(crate) fn find_value_response_with_nodes(txid: i32, nodes4: Option<Vec<NodeInfo>>, nodes6: Option<Vec<NodeInfo>>)-> Message {
    let body = Body::FindValueResponse(
        FindValueResponse::with_nodes(nodes4, nodes6)
    );
    Message::new(Kind::Response, Method::FindValue, txid, Some(body))
}

pub(crate) fn find_value_response(txid: i32, value: Value) -> Message {
    let body = Body::FindValueResponse(
        FindValueResponse::with_value(value)
    );
    Message::new(Kind::Response, Method::FindValue, txid, Some(body))
}

pub(crate) fn store_value_request(value: Value, token: i32, expected_seq: i32) -> Message {
    let body = Body::StoreValueRequest(
        StoreValueRequest::new(value, token, expected_seq)
    );
    Message::new(Kind::Request, Method::StoreValue, next_txid(), Some(body))
}

pub(crate) fn store_value_response(txid: i32) -> Message {
    Message::new(Kind::Response, Method::StoreValue, txid, None)
}

pub(crate) fn announce_peer_request(peer: PeerInfo, token: i32, expected_seq: i32) -> Message {
    let body = Body::AnnouncePeerRequest(
        AnnouncePeerRequest::new(peer, token, Some(expected_seq))
    );
    Message::new(Kind::Request, Method::AnnouncePeer, next_txid(), Some(body))
}

pub(crate) fn announce_peer_response(txid: i32) -> Message {
    Message::new(Kind::Response, Method::AnnouncePeer, txid, None)
}

pub(crate) fn error_msg(method: Method, txid: i32, code: i32, description: String) -> Message {
    let body = Body::Error(
        ErrorBody::new(code, description)
    );
    Message::new(Kind::Error, method, txid, Some(body))
}

impl fmt::Display for Message {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let json = serde_json::to_string(&self)
            .map_err(|_| fmt::Error)?;
        write!(f, "{}", json)
    }
}

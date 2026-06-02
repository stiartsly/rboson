use std::{
    fmt,
    net::SocketAddr,
    result::Result as SResult,
    sync::{Arc, Mutex, atomic::{AtomicI32, Ordering}}
};
use serde_cbor::value::{Value as CborValue, from_value};
use serde::{
    Deserialize,
    Serialize,
    de::{self, Deserializer, MapAccess, Visitor, IgnoredAny},
    ser::{SerializeMap, Serializer}
};

use crate::{
    Id,
    Value,
    NodeInfo,
    PeerInfo,
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
    fn from_value(kind: Kind, method: Method, value: CborValue) -> Result<Option<Self>, String> {
        match kind {
            Kind::Error => from_value::<ErrorBody>(value)
                .map(Body::Error)
                .map(Some)
                .map_err(|e| format!("failed to decode error body: {}", e)),
            Kind::Request => match method {
                Method::Ping => Ok(None),
                Method::FindNode => from_value::<FindNodeRequest>(value)
                    .map(Body::FindNodeRequest)
                    .map(Some)
                    .map_err(|e| format!("failed to decode find_node request: {}", e)),
                Method::AnnouncePeer => from_value::<AnnouncePeerRequest>(value)
                    .map(Body::AnnouncePeerRequest)
                    .map(Some)
                    .map_err(|e| format!("failed to decode announce_peer request: {}", e)),
                Method::FindPeer => from_value::<FindPeerRequest>(value)
                    .map(Body::FindPeerRequest)
                    .map(Some)
                    .map_err(|e| format!("failed to decode find_peer request: {}", e)),
                Method::StoreValue => from_value::<StoreValueRequest>(value)
                    .map(Body::StoreValueRequest)
                    .map(Some)
                    .map_err(|e| format!("failed to decode store_value request: {}", e)),
                Method::FindValue => from_value::<FindValueRequest>(value)
                    .map(Body::FindValueRequest)
                    .map(Some)
                    .map_err(|e| format!("failed to decode find_value request: {}", e)),
                Method::Unknown => Err("invalid unknown request".into()),
            },
            Kind::Response => match method {
                Method::Ping | Method::AnnouncePeer | Method::StoreValue => Ok(None),
                Method::FindNode => from_value::<FindNodeResponse>(value)
                    .map(Body::FindNodeResponse)
                    .map(Some)
                    .map_err(|e| format!("failed to decode find_node response: {}", e)),
                Method::FindPeer => from_value::<FindPeerResponse>(value)
                    .map(Body::FindPeerResponse)
                    .map(Some)
                    .map_err(|e| format!("failed to decode find_peer response: {}", e)),
                Method::FindValue => from_value::<FindValueResponse>(value)
                    .map(Body::FindValueResponse)
                    .map(Some)
                    .map_err(|e| format!("failed to decode find_value response: {}", e)),
                Method::Unknown => Err("invalid unknown response".into()),
            },
        }
    }
}

impl Serialize for Body {
    fn serialize<S>(&self, se: S) -> SResult<S::Ok, S::Error>
    where S: Serializer
    {
        match self {
            Body::FindNodeRequest(body) => body.serialize(se),
            Body::FindNodeResponse(body) => body.serialize(se),
            Body::FindPeerRequest(body) => body.serialize(se),
            Body::FindPeerResponse(body) => body.serialize(se),
            Body::FindValueRequest(body) => body.serialize(se),
            Body::FindValueResponse(body) => body.serialize(se),
            Body::AnnouncePeerRequest(body) => body.serialize(se),
            Body::StoreValueRequest(body) => body.serialize(se),
            Body::Error(body) => body.serialize(se),
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

pub(crate) struct Message {
    nodeid  : Option<Id>,        // The DHT node Id of the message sender.
    kind    : Kind,
    method  : Method,
    txid    : i32,
    ver     : i32,

    body    : Option<Body>,

    associated_call : Option<Arc<Mutex<RpcCall>>>,
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

    pub(crate) fn associated_call(&self) -> Option<Arc<Mutex<RpcCall>>> {
        self.associated_call.clone()
    }

    pub(crate) fn set_associated_call(&mut self, call: Arc<Mutex<RpcCall>>) {
        self.associated_call = Some(call);
    }

    pub(crate) fn remote_id(&self) -> &Id {
        match self.remote_id.as_ref() {
            Some(id) => id,
            None => panic!("no remote ID associated with this message")
        }
    }

    pub(crate) fn remote_addr(&self) -> &SocketAddr {
        match self.remote_addr.as_ref() {
            Some(addr) => addr,
            None => panic!("no remote address associated with this message")
        }
    }

    pub(crate) fn set_remote(&mut self, id: Id, addr: SocketAddr) -> &mut Self {
        self.remote_id = Some(id);
        self.remote_addr = Some(addr);
        self
    }
}

impl Serialize for Message {
    fn serialize<S>(&self, se: S) -> SResult<S::Ok, S::Error>
    where S: Serializer
    {
        let mut s = se.serialize_map(None)?;
        s.serialize_entry("y", &(self.composite_type()))?;
        s.serialize_entry("t", &self.txid)?;
        s.serialize_entry("v", &self.ver)?;
        if let Some(body) = self.body.as_ref() {
            match self.kind() {
                Kind::Request => s.serialize_entry("q", body)?,
                Kind::Response => s.serialize_entry("r", body)?,
                Kind::Error => s.serialize_entry("e", body)?,
            }
        }
        s.end()
    }
}

impl<'de> Deserialize<'de> for Message {
    fn deserialize<D>(de: D) -> SResult<Self, D::Error>
    where D: Deserializer<'de>
    {
        enum Field {
            Type,               // "y"  - i32
            TransactionId,      // "t"  - i32
            Version,            // "v"  - i32
            Request,            // "q"  - Request.
            Response,           // "r"  - Response,
            Error,              // "e"  - Error
            Ignore              // Ignore unknown fields
        }

        impl<'de> Deserialize<'de> for Field {
            fn deserialize<D>(de: D) -> SResult<Field, D::Error>
            where D: Deserializer<'de>,
            {
                let key = String::deserialize(de)?;
                match key.as_str() {
                    "y"     => Ok(Field::Type),
                    "t"     => Ok(Field::TransactionId),
                    "v"     => Ok(Field::Version),
                    "q"     => Ok(Field::Request),
                    "r"     => Ok(Field::Response),
                    "e"     => Ok(Field::Error),
                    _       => Ok(Field::Ignore),
                }
            }
        }

        struct FieldVisitor;
        impl<'de> Visitor<'de> for FieldVisitor {
            type Value = Message;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a Message struct")
            }

            fn visit_map<V>(self, mut map: V) -> SResult<Self::Value, V::Error>
            where V: MapAccess<'de>
            {
                let mut kind: Option<Kind> = None;
                let mut method: Option<Method> = None;
                let mut txid: Option<i32> = None;
                let mut ver: Option<i32> = None;
                let mut body: Option<Body> = None;

                while let Some(key) = map.next_key::<Field>()? {
                    match key {
                        Field::Type => {
                            if kind.is_some() || method.is_some() {
                                return Err(de::Error::duplicate_field("y"));
                            }
                            let type_ = map.next_value::<i32>()?;
                            if !Kind::is_valid(type_) || !Method::is_valid(type_) {
                                return Err(de::Error::custom(format!(
                                    "invalid message kind/method composite type: {}", type_
                                )));
                            }
                            kind = Some(Kind::from(type_));
                            method = Some(Method::from(type_));
                        },
                        Field::TransactionId => {
                            if txid.is_some() {
                                return Err(de::Error::duplicate_field("t"));
                            }
                            let current_txid = map.next_value::<i32>()?;
                            if current_txid == 0 {
                                return Err(de::Error::custom("invalid '[t]xid' field: should be non-zero integer"));
                            }
                            txid = Some(current_txid);
                        }
                        Field::Version => {
                            if ver.is_some() {
                                return Err(de::Error::duplicate_field("v"));
                            }
                            ver = Some(map.next_value::<i32>()?);
                        }
                        Field::Request => {
                            let Some(kind) = kind else {
                                return Err(de::Error::custom("Field 'q' must come after field 'y'"));
                            };
                            let Some(method) = method else {
                                return Err(de::Error::custom("Field 'q' must come after field 'y'"));
                            };
                            if kind != Kind::Request {
                                return Err(de::Error::custom("Field 'q' is only valid for request messages"));
                            }
                            if body.is_some() {
                                return Err(de::Error::duplicate_field("q"));
                            }
                            let value = map.next_value::<CborValue>()?;
                            body = Body::from_value(kind, method, value)
                                .map_err(de::Error::custom)?;

                        },
                        Field::Response => {
                            let Some(kind) = kind else {
                                return Err(de::Error::custom("Field 'r' must come after field 'y'"));
                            };
                            let Some(method) = method else {
                                return Err(de::Error::custom("Field 'r' must come after field 'y'"));
                            };
                            if kind != Kind::Response {
                                return Err(de::Error::custom("Field 'r' is only valid for response messages"));
                            }
                            if body.is_some() {
                                return Err(de::Error::duplicate_field("r"));
                            }
                            let value = map.next_value::<CborValue>()?;
                            body = Body::from_value(kind, method, value)
                                .map_err(de::Error::custom)?;
                        },
                        Field::Error => {
                            let Some(kind) = kind else {
                                return Err(de::Error::custom("Field 'e' must come after field 'y'"));
                            };
                            if kind != Kind::Error {
                                return Err(de::Error::custom("Field 'e' is only valid for error messages"));
                            };
                            if body.is_some() {
                                return Err(de::Error::duplicate_field("e"));
                            }
                            let value = map.next_value::<CborValue>()?;
                            body = Body::from_value(kind, Method::Unknown, value)
                                .map_err(de::Error::custom)?;
                        },
                        _ => _ = map.next_value::<IgnoredAny>()?,
                    }
                }

                let mut msg = Message::new(
                    kind.ok_or_else(|| de::Error::missing_field("y"))?,
                    method.ok_or_else(|| de::Error::missing_field("y"))?,
                    txid.ok_or_else(|| de::Error::missing_field("t"))?,
                    body
                );
                msg.ver = ver.ok_or_else(|| de::Error::missing_field("v"))?;
                Ok(msg)
            }
        }
        de.deserialize_map(FieldVisitor)
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

pub(crate) fn error(method: Method, txid: i32, code: i32, description: String) -> Message {
    let body = Body::Error(
        ErrorBody::new(code, description)
    );
    Message::new(Kind::Error, method, txid, Some(body))
}

impl fmt::Display for Message {
    fn fmt(&self, _f: &mut fmt::Formatter) -> fmt::Result {
        unimplemented!()
    }
}

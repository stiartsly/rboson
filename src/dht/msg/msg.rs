use std::fmt;
use std::result::Result as SResult;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicI32, Ordering};
use serde_cbor::value::{
    to_value,
    Value as CborValue
};
use serde::{
    Deserialize,
    Serialize,
    de::{self, Deserializer, MapAccess, Visitor, IgnoredAny},
    ser::{SerializeMap, Serializer}
};

use crate::{
    Id,
    NodeInfo,
    PeerInfo,
    Value,
    core::version,
    dht::rpc::rpccall::RpcCall
};
use crate::dht::msg::{
    error::Error,
    find_node_req::FindNodeRequest,
    find_node_rsp::FindNodeResponse,
    find_peer_req::FindPeerRequest,
    find_peer_rsp::FindPeerResponse,
    find_value_req::FindValueRequest,
    find_value_rsp::FindValueResponse,
    announce_peer_req::AnnouncePeerRequest,
    store_value_req::StoreValueRequest,
};

#[derive(PartialEq, Clone, Copy)]
pub(crate) enum Kind {
    Error = 0,
    Request = 0x20,
    Response = 0x40,
}

impl Kind {
    const MASK: i32 = 0xE0;
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
    pub(crate) fn is_valid(_type: i32) -> bool {
        let method = _type & Self::MASK;
        method >= 0 && method <= 0x06
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
    FindNodeReq(FindNodeRequest),
    FindNodeRsp(FindNodeResponse),
    FindPeerReq(FindPeerRequest),
    FindPeerRsp(FindPeerResponse),
    FindValueReq(FindValueRequest),
    FindValueRsp(FindValueResponse),
    AnnouncePeerReq(AnnouncePeerRequest),
    StoreValueReq(StoreValueRequest),
    Error(Error),
}

impl Body {
    fn from_value(_kind: Kind, _: Option<Method>, _: &CborValue) -> Option<Self> {
        unimplemented!()
    }

    fn to_value(&self) -> Option<serde_cbor::Value> {
        match self {
            Body::FindNodeReq(req) => to_value(req).ok(),
            Body::FindPeerReq(req) => to_value(req).ok(),
            Body::FindValueReq(req) => to_value(req).ok(),
            Body::StoreValueReq(req) => to_value(req).ok(),
            Body::AnnouncePeerReq(req) => to_value(req).ok(),
            Body::FindNodeRsp(rsp) => to_value(rsp).ok(),
            Body::FindPeerRsp(rsp) => to_value(rsp).ok(),
            Body::FindValueRsp(rsp) => to_value(rsp).ok(),
            Body::Error(err) => to_value(err).ok(),
        }
    }
}

pub(crate) struct Message {
    id:     Option<Id>,        // The DHT node Id of the message sender.

    kind    : Kind,
    method  : Method,
    txid    : i32,
    ver     : i32,
    body    : Option<Body>,

    associated_call: Option<Arc<Mutex<RpcCall>>>,

    remote_addr : Option<SocketAddr>,
    remote_id: Option<Id>,
}

impl Message {
    pub(crate) const MIN_BYTES: usize = 10;

    fn new(kind: Kind, method: Method,  txid: i32, body: Option<Body>) -> Self {
        Self {
            id: None,
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

    fn next_txid() -> i32 {
        static TXID_COUNTER: AtomicI32 = AtomicI32::new(1);
        let txid = TXID_COUNTER.fetch_add(1, Ordering::SeqCst);
        if txid == 0 {
            TXID_COUNTER.fetch_add(1, Ordering::SeqCst)
        } else {
            txid
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

    pub(crate) fn is_rsp(&self) -> bool {
        self.kind == Kind::Response
    }

    pub(crate) fn is_err(&self) -> bool {
        self.kind == Kind::Error
    }

    pub(crate) fn id(&self) -> &Id {
        self.id.as_ref().expect("Id not set")
    }

    pub(crate) fn set_id(&mut self, id: Id) {
        self.id = Some(id)
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

    pub(crate) fn readable_version(&self) -> String {
        version::format_version(self.ver)
    }

    pub(crate) fn associated_call(&self) -> Option<Arc<Mutex<RpcCall>>> {
        self.associated_call.as_ref().map(|v| v.clone())
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

    pub(crate) fn ping_req() -> Self {
        Self::new(
            Kind::Request,
            Method::Ping,
            Self::next_txid(),
            None
        )
    }

    pub(crate) fn ping_rsp(txid: i32) -> Self {
        Self::new(
            Kind::Response,
            Method::Ping,
            txid,
            None
        )
    }

    pub(crate) fn find_node_req(
        target: Id,
        want4: bool,
        want6: bool,
        want_token:bool
    ) -> Self {
        let req = FindNodeRequest::new(
            target, want4, want6, want_token
        );
        Self::new(
            Kind::Request,
            Method::FindNode,
            Self::next_txid(),
            Some(Body::FindNodeReq(req))
        )
    }

    pub(crate) fn find_node_rsp(
        txid: i32,
        nodes4: Option<Vec<NodeInfo>>,
        nodes6: Option<Vec<NodeInfo>>,
        token: i32
    ) -> Self {
        let rsp = FindNodeResponse::new(
            nodes4, nodes6, token
        );
        Self::new(
            Kind::Response,
            Method::FindNode,
            txid,
            Some(Body::FindNodeRsp(rsp))
        )
    }

    pub(crate) fn find_peer_req(
        target: Id,
        want4: bool,
        want6: bool,
        expected_seq: i32,
        expected_count: i32,
    ) -> Self {
        let req = FindPeerRequest::new(
            target, want4, want6, expected_seq, expected_count
        );
        Self::new(
            Kind::Request,
            Method::FindPeer,
            Self::next_txid(),
            Some(Body::FindPeerReq(req))
        )
    }

    pub(crate) fn find_peer_rsp_with_nodes(
        txid: i32,
        nodes4: Option<Vec<NodeInfo>>,
        nodes6: Option<Vec<NodeInfo>>
    ) -> Self {
        let body = Body::FindPeerRsp(
            FindPeerResponse::with_nodes(nodes4, nodes6)
        );
        Self::new(
            Kind::Response,
            Method::FindPeer,
            txid,
            Some(body)
        )
    }

    pub(crate) fn find_peer_rsp(
        txid: i32,
        peers: Vec<PeerInfo>
    ) -> Self {
        let body = Body::FindPeerRsp(
            FindPeerResponse::with_peers(peers)
        );
        Self::new(
            Kind::Response,
            Method::FindPeer,
            txid,
            Some(body)
        )
    }

    pub(crate) fn find_value_req(
        target: Id,
        want4: bool,
        want6: bool,
        expected_seq: i32,
    ) -> Self {
        let req = FindValueRequest::new(
            target, want4, want6, expected_seq
        );
        Self::new(
            Kind::Request,
            Method::FindValue,
            Self::next_txid(),
            Some(Body::FindValueReq(req))
        )
    }

    pub(crate) fn find_value_rsp_with_nodes(
        txid: i32,
        nodes4: Option<Vec<NodeInfo>>,
        nodes6: Option<Vec<NodeInfo>>
    ) -> Self {
        let body = Body::FindValueRsp(
            FindValueResponse::with_nodes(nodes4, nodes6)
        );
        Self::new(
            Kind::Response,
            Method::FindValue,
            txid,
            Some(body)
        )
    }

    pub(crate) fn find_value_rsp(
        txid: i32,
        value: Value
    ) -> Self {
        let body = Body::FindValueRsp(
            FindValueResponse::with_value(value)
        );
        Self::new(
            Kind::Response,
            Method::FindValue,
            txid,
            Some(body)
        )
    }

    pub(crate) fn store_value_req(
        value: Value,
        token: i32,
        expected_seq: i32
    ) -> Self {
        let req = StoreValueRequest::new(
            value, token, expected_seq
        );
        Self::new(
            Kind::Request,
            Method::StoreValue,
            Self::next_txid(),
            Some(Body::StoreValueReq(req))
        )
    }

    pub(crate) fn store_value_rsp(txid: i32) -> Self {
        Self::new(
            Kind::Response,
            Method::StoreValue,
            txid,
            None
        )
    }

    pub(crate) fn announce_peer_req(
        peer: PeerInfo,
        token: i32,
        expected_seq: i32,
    ) -> Self {
        let req = AnnouncePeerRequest::new(
            peer, token, Some(expected_seq)
        );
        Self::new(
            Kind::Request,
            Method::AnnouncePeer,
            Self::next_txid(),
            Some(Body::AnnouncePeerReq(req))
        )
    }

    pub(crate) fn announce_peer_rsp(txid: i32) -> Self {
        Self::new(
            Kind::Response,
            Method::AnnouncePeer,
            txid,
            None
        )
    }

    pub(crate) fn error(
        method: Method,
        txid: i32,
        code: i32,
        msg: String
    ) -> Self {
        let error = Error::new(code, msg);
        Self::new(
            Kind::Error,
            method,
            txid,
            Some(Body::Error(error))
        )
    }
}

impl std::fmt::Debug for Message {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        panic!()
    }
}

impl Serialize for Message {
    fn serialize<S>(&self, se: S) -> SResult<S::Ok, S::Error>
    where
        S: Serializer
    {
        let mut s = se.serialize_map(None)?;
        s.serialize_entry("y", &(self.composite_type()))?;
        s.serialize_entry("t", &self.txid)?;
        s.serialize_entry("v", &self.ver)?;
        if let Some(body) = self.body.as_ref() {
            match self.kind() {
                Kind::Request => s.serialize_key("q")?,
                Kind::Response => s.serialize_key("r")?,
                Kind::Error => s.serialize_key("e")?,
            }
            let value = body.to_value().ok_or_else(
                || serde::ser::Error::custom("Failed to serialize message body")
            )?;
            s.serialize_value(&value)?;
        }
        s.end()
    }
}

impl<'de> Deserialize<'de> for Message {
    fn deserialize<D>(de: D) -> SResult<Self, D::Error>
    where
        D: Deserializer<'de>
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
            where
                D: Deserializer<'de>,
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
                formatter.write_str("Message field")
            }

            fn visit_map<V>(self, mut map: V) -> SResult<Self::Value, V::Error>
            where
                V: MapAccess<'de>
            {

                let mut kind: Option<Kind> = None;
                let mut method: Option<Method> = None;
                let mut txid: i32 = 0;
                let mut ver: i32 = 0;
                let mut body: Option<Body> = None;

                while let Some(key) = map.next_key::<Field>()? {
                    match key {
                        Field::Type => {
                            let type_ = map.next_value()?;
                                let Some(type_) = type_ else {
                                return Err(de::Error::missing_field("y"));
                            };

                            if !Kind::is_valid(type_) || !Method::is_valid(type_) {
                                return Err(de::Error::custom(format!(
                                    "Invalid message kind {} or method {}", Kind::from(type_), Method::from(type_)
                                )));
                            }
                            kind = Some(Kind::from(type_));
                            method = Some(Method::from(type_));
                        },
                        Field::TransactionId => txid = map.next_value::<i32>()?,
                        Field::Version => ver = map.next_value()?,
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

                            body = Body::from_value(kind, Some(method), &map.next_value::<CborValue>()?);

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
                            body = Body::from_value(kind, Some(method), &map.next_value::<CborValue>()?);
                        },
                        Field::Error => {
                            let Some(kind) = kind else {
                                return Err(de::Error::custom("Field 'e' must come after field 'y'"));
                            };
                            if kind != Kind::Error {
                                return Err(de::Error::custom("Field 'e' is only valid for error messages"));
                            };
                            body = Body::from_value(kind, None, &map.next_value::<CborValue>()?);
                        },
                        _ => _ = map.next_value::<IgnoredAny>()?,
                    }
                }

                Ok(Message::new(
                    kind.ok_or_else(|| de::Error::missing_field("y"))?,
                    method.ok_or_else(|| de::Error::missing_field("y"))?,
                    txid,
                    body
                ))
            }
        }
        de.deserialize_map(FieldVisitor)
    }
}

impl fmt::Display for Message {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        unimplemented!()
    }
}

use std::fmt;
use std::rc::Rc;
use std::any::Any;
use ciborium::Value as CVal;

use crate::{
    cryptobox,
    Id,
    NodeInfo,
    Value,
    Error,
    core::{
        Result,
        value::PackBuilder
    }
};

use super::{
    msg::{
        Kind, Method, Msg,
        Data as MsgData
    },
    lookup_rsp::{
        Msg as LookupResponse,
        Data as LookupResponseData
    },
};

pub(crate) struct Message {
    base_data   : MsgData,
    lookup_data : LookupResponseData,

    value: Option<Rc<Value>>,
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
                    let mut pk = None;
                    let mut rec = None;
                    let mut nonce = None;
                    let mut sig = None;
                    let mut data = None;
                    let mut seq = -1;

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
                            "k" =>   pk  = Some(Id::from_cbor(v)?),    // public_key
                            "rec" => rec = Some(Id::from_cbor(v)?),    // recipient.
                            "n" => nonce = Some(cryptobox::Nonce::try_from(v.as_bytes()?.as_slice()).unwrap()), // nonce.
                            "s" =>   sig = Some(v.as_bytes()?),                 // signature.
                            "seq" => seq = v.as_integer()?.try_into().unwrap(), // sequence number
                            "v" =>  data = Some(v.as_bytes()?.clone()),         // data
                            _ => return None
                        }
                    }
                    self.value = data.map_or(None, |v|
                        Some(Rc::new(PackBuilder::new(v)
                            .with_pk(pk)
                            .with_sk(None)
                            .with_rec(rec)
                            .with_nonce(nonce.take())
                            .with_sig(sig.map(|v|v.to_vec()))
                            .with_seq(seq)
                            .build()
                    )));
                },
                _ => return None,
            }
        }
        Some(())
    }

    fn ser(&self) -> CVal {
        let mut val = vec![];
        if let Some(value) = self.value.as_ref() {
            value.public_key().map(|pk| val.push((
                CVal::Text(String::from("k")),
                CVal::Bytes(pk.as_bytes().into())
            )));

            value.recipient().map(|rec| val.push((
                CVal::Text(String::from("rec")),
                CVal::Bytes(rec.as_bytes().into()),
            )));

            value.nonce().map(|nonce| val.push((
                CVal::Text("n".to_string()),
                CVal::Bytes(nonce.as_bytes().to_vec()),
            )));

            if value.sequence_number() >= 0 {
                val.push((
                    CVal::Text(String::from("seq")),
                    CVal::Integer(value.sequence_number().into()),
                ));
            }

            value.signature().map(|sig| val.push((
                CVal::Text(String::from("s")),
                CVal::Bytes(sig.to_vec()),
            )));

            val.push((
                CVal::Text(String::from("v")),
                CVal::Bytes(value.data().to_vec())
            ));
        }

        let mut rsp = LookupResponse::to_cbor(self);
        if let Some(map) = rsp.as_map_mut() {
            map.extend_from_slice(&val);
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
    fn data(&self) -> &LookupResponseData {
        &self.lookup_data
    }

    fn data_mut(&mut self) -> &mut LookupResponseData {
        &mut self.lookup_data
    }
}

impl Message {
    pub(crate) fn new() -> Self {
        Self {
            lookup_data: LookupResponseData::new(),
            base_data: MsgData::new(
                Kind::Response,
                Method::FindValue,
                0
            ),
            value: None,
        }
    }

    pub(crate) fn value(&self) -> Option<Rc<Value>> {
        self.value.as_ref().map(|v|v.clone())
    }

    pub(crate) fn populate_value(&mut self, value: Rc<Value>) {
        self.value = Some(value)
    }
}

impl TryFrom<CVal> for Box<Message> {
    type Error = Error;
    fn try_from(input: CVal) -> Result<Box<Message>> {
        let mut msg = Self::new(Message::new());
        if let None =  msg.from_cbor(&input) {
                return Err(Error::Protocol(
                    format!("Invalid cobor value for find_value_rsp message")));
        }
        Ok(msg)
    }
}

impl fmt::Display for Message {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(val) = self.value.as_ref() {
            write!(f, "{}", val)?;
        }
        Ok(())
    }
}

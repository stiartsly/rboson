use std::fmt;
use std::rc::Rc;
use std::any::Any;
use ciborium::Value as CVal;

use crate::{
    Id,
    Value,
    cryptobox::Nonce,
    Error,
    error::Result
};

use crate::core::{
    version,
    value::PackBuilder
};

use super::msg::{
    Kind, Method, Msg,
    Data as MsgData
};

pub(crate) struct Message {
    base_data: MsgData,

    token: i32,
    expected_seq: i32,
    value: Option<Rc<Value>>, // must contain a value.
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
                "q" => {
                    let mut pkey = None;
                    let mut rec  = None;
                    let mut nonce= None;
                    let mut sig  = None;
                    let mut data = None;
                    let mut seq  = 0;

                    let map = v.as_map()?;
                    for (k,v) in map {
                        let k = k.as_text()?;
                        match k {
                            "k" =>  pkey = Some(Id::from_cbor(v)?),     // publickey
                            "rec" => rec = Some(Id::from_cbor(v)?),     // recipient
                            "n" => nonce = Some(Nonce::try_from(v.as_bytes()?.as_slice()).unwrap()),  // nonce
                            "s" =>   sig = Some(v.as_bytes()?),         // signature.
                            "seq" => seq = v.as_integer()?.try_into().unwrap(), // sequence number
                            "cas" => self.expected_seq = v.as_integer()?.try_into().unwrap(),
                            "tok" => self.token = v.as_integer()?.try_into().unwrap(), // token
                            "v" =>  data = Some(v.as_bytes()?.clone()), // value
                            _ => return None,
                        }
                    }

                    assert!(data.is_some());
                    if data.is_none() {
                        return None;
                    }

                    self.value = Some(Rc::new({
                        PackBuilder::new(data.unwrap())
                            .with_pk(pkey)
                            .with_sk(None)
                            .with_rec(rec)
                            .with_nonce(nonce.take())
                            .with_sig(sig.as_ref().map(|v|v.to_vec()))
                            .with_seq(seq)
                            .build()
                    }));
                },
                _ => return None,
            }
        }
        Some(())
    }

    fn ser(&self) -> CVal {
        let mut val = vec![(
            CVal::Text(String::from("tok")),
            CVal::Integer(self.token.into())
        )];

        let value = self.value.as_ref().unwrap();
        value.public_key().map(|pk| val.push((
            CVal::Text(String::from("k")),
            CVal::Bytes(pk.as_bytes().into())
        )));

        value.recipient().map(|rec| val.push((
            CVal::Text(String::from("rec")),
            CVal::Bytes(rec.as_bytes().into())
        )));

        value.nonce().map(|nonce| val.push((
            CVal::Text(String::from("n")),
            CVal::Bytes(nonce.as_bytes().into())
        )));

        value.signature().map(|sig| val.push((
            CVal::Text(String::from("s")),
            CVal::Bytes(sig.into()),
        )));

        if value.sequence_number() >= 0 {
            val.push((
                CVal::Text(String::from("seq")),
                CVal::Integer(value.sequence_number().into())
            ));
        }

        if self.expected_seq >= 0 {
            val.push((
                CVal::Text(String::from("cas")),
                CVal::Integer(self.expected_seq.into())
            ));
        }

        val.push((
            CVal::Text(String::from("v")),
            CVal::Bytes(value.data().into()),
        ));

        let mut root = Msg::to_cbor(self);
        root.as_map_mut().unwrap().push((
            CVal::Text(String::from("q")),
            CVal::Map(val)
        ));
        root
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl Message {
    pub(crate) fn new(value: Option<Rc<Value>>) -> Self {
        Self {
            base_data: MsgData::new(
                Kind::Request,
                Method::StoreValue,
                0
            ),
            token: 0,
            expected_seq: -1,
            value,
        }
    }

    pub(crate) fn token(&self) -> i32 {
        self.token
    }

    pub(crate) fn with_token(&mut self, token: i32) {
        self.token = token
    }

    pub(crate) fn value(&self) -> Rc<Value> {
        assert!(self.value.is_some());
        self.value.as_ref().unwrap().clone()
    }
}

impl Into<CVal> for Message {
    fn into(self) -> CVal {
        let mut val = vec![(
            CVal::Text(String::from("tok")),
            CVal::Integer(self.token.into())
        )];

        let value = self.value.as_ref().unwrap();
        value.public_key().map(|pk| val.push((
            CVal::Text(String::from("k")),
            CVal::Bytes(pk.as_bytes().into())
        )));

        value.recipient().map(|rec| val.push((
            CVal::Text(String::from("rec")),
            CVal::Bytes(rec.as_bytes().into())
        )));

        value.nonce().map(|nonce| val.push((
            CVal::Text(String::from("n")),
            CVal::Bytes(nonce.as_bytes().into())
        )));

        value.signature().map(|sig| val.push((
            CVal::Text(String::from("s")),
            CVal::Bytes(sig.into()),
        )));

        if value.sequence_number() >= 0 {
            val.push((
                CVal::Text(String::from("seq")),
                CVal::Integer(value.sequence_number().into())
            ));
        }

        if self.expected_seq >= 0 {
            val.push((
                CVal::Text(String::from("cas")),
                CVal::Integer(self.expected_seq.into())
            ));
        }

        val.push((
            CVal::Text(String::from("v")),
            CVal::Bytes(value.data().into()),
        ));

        let mut root = Msg::to_cbor(&self);
        root.as_map_mut().unwrap().push((
            CVal::Text(String::from("q")),
            CVal::Map(val)
        ));
        root
    }
}

impl TryFrom<CVal> for Box<Message> {
    type Error = Error;
    fn try_from(input: CVal) -> Result<Box<Message>> {
        let mut msg = Self::new(Message::new(None));
        if let None =  msg.from_cbor(&input) {
                return Err(Error::Protocol(
                    format!("Invalid cobor value for store_value_req message")))
        }
        Ok(msg)
    }
}

impl fmt::Display for Message {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f,
            "y:{},m:{},t:{},q:{{",
            self.kind(),
            self.method(),
            self.txid() as u32
        )?;

        let val = self.value.as_ref().unwrap();
        if val.is_mutable() {
            write!(f, ",k:{}", val.public_key().unwrap())?;
            if val.is_encrypted() {
                write!(f, ",rec:{}", val.recipient().unwrap())?;
            }
            write!(f, ",n:{}",
                hex::encode(val.nonce().unwrap().as_bytes())
            )?;
            if val.sequence_number() >= 0 {
                write!(f, ",seq:{}", val.sequence_number())?;
            }
            write!(f, "sig:{}",
                hex::encode(val.signature().unwrap())
            )?;
            if self.expected_seq >= 0 {
                write!(f, ",cas:{}", self.expected_seq)?;
            }
            write!(f, ",")?;
        }

        write!(f, "tok:{}", self.token as u32)?;
        write!(f,
            "}},v:{}",
            version::formatted_version(self.ver())
        )?;
        Ok(())
    }
}

use std::fmt;
use std::rc::Rc;
use std::any::Any;
use ciborium::Value as CVal;

use crate::{
    Id,
    Error,
    core::{version, Result}
};

use super::{
    msg::{
        Kind, Method, Msg,
        Data as MsgData
    },
    lookup_req::{
        Msg as LookupRequest,
        Data as LookupRequestData
    },
};

pub(crate) struct Message {
    base_data   : MsgData,
    lookkup_data: LookupRequestData,

    seq         : i32,
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
        for (k, v) in root {
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
                            "w" => {
                                let want: i32 = v.as_integer()?.try_into().unwrap();
                                self.with_want4((want & 0x01) != 0);
                                self.with_want6((want & 0x02) != 0);
                                self.with_want_token((want & 0x04) != 0);
                            },
                            "t" => {
                                self.with_target(Rc::new(Id::from_cbor(v)?));
                            },
                            "seq" => {
                                let seq = v.as_integer()?.try_into().unwrap();
                                self.seq = if seq > 0 { seq } else {0};
                            }
                            _ => return None,
                        }
                    }
                },
                _ => return None,
            }
        }
        Some(())
    }

    fn ser(&self) -> CVal {
        let mut val = LookupRequest::to_cbor(self);
        if let Some(map) = val.as_map_mut() {
            map.push((
                CVal::Text(String::from("seq")),
                CVal::Integer(self.seq.into())
            ));
        }

        let mut root = Msg::to_cbor(self);
        if let Some(map) = root.as_map_mut() {
            map.push((
                CVal::Text(String::from(Kind::Request.to_key())),
                val
            ));
        }
        root
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl LookupRequest for Message {
    fn data(&self) -> &LookupRequestData {
        &self.lookkup_data
    }

    fn data_mut(&mut self) -> &mut LookupRequestData {
        &mut self.lookkup_data
    }
}

impl Message {
    pub(crate) fn new() -> Self {
        Self {
            lookkup_data: LookupRequestData::new(true),
            base_data: MsgData::new(
                Kind::Request,
                Method::FindValue,
                0
            ),
            seq: -1
        }
    }

    pub(crate) fn seq(&self) -> i32 {
        self.seq
    }

    pub(crate) fn with_seq(&mut self, seq: i32) {
        self.seq = seq
    }
}

impl TryFrom<CVal> for Box<Message> {
    type Error = Error;
    fn try_from(input: CVal) -> Result<Box<Message>> {
        let mut msg = Self::new(Message::new());
        if let None =  msg.from_cbor(&input) {
                return Err(Error::Protocol(
                    format!("Invalid cobor value for find_value_req message")));
        }
        Ok(msg)
    }
}

impl fmt::Display for Message {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f,
            "y:{},m:{},t:{},q:{{t:{},w:{}}}",
            self.kind(),
            self.method(),
            self.txid() as u32,
            self.target(),
            self.want()
        )?;
        if self.seq >= 0 {
            write!(f, ",seq:{}", self.seq)?;
        }

        write!(f,
            ",v:{}",
            version::format_version(self.ver())
        )?;
        Ok(())
    }
}

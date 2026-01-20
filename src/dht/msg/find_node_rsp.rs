use std::fmt;
use std::rc::Rc;
use std::any::Any;
use ciborium::Value as CVal;

use crate::{
    NodeInfo,
    Error,
    core::{version, Result}
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
                "r" => {
                    let map = v.as_map()?;
                    for (k, v) in map {
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
                            }
                            _ => return None
                        }
                    }
                },
                _ => return None,
            }
        }
        Some(())
    }

    fn ser(&self) -> CVal {
        let mut root = Msg::to_cbor(self);
        if let Some(map) = root.as_map_mut() {
            map.push(
                (CVal::Text(String::from("r")),
                LookupResponse::to_cbor(self)
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
        Message {
            lookup_data: LookupResponseData::new(),
            base_data: MsgData::new(
                Kind::Response,
                Method::FindNode,
                0
            ),
        }
    }
}

impl TryFrom<CVal> for Box<Message> {
    type Error = Error;
    fn try_from(input: CVal) -> Result<Box<Message>> {
        let mut msg = Self::new(Message::new());
        if let None =  msg.from_cbor(&input) {
            return Err(Error::Protocol(
                format!("Invalid cobor value for find_node_rsp message")));
        }
        Ok(msg)
    }
}

impl fmt::Display for Message {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f,
            "y:{},m:{},t:{},r:{{",
            self.kind(),
            self.method(),
            self.txid() as u32
        )?;

        if let Some(nodes4) = self.nodes4() {
            let mut first = true;
            if !nodes4.is_empty() {
                write!(f, "n4:")?;
                for item in nodes4.iter() {
                    if !first {
                        first = false;
                        write!(f, ",")?;
                    }
                    write!(f, "[{}]", item)?;
                }
            }
        }

        if let Some(nodes6) = self.nodes6() {
            let mut first = true;
            if !nodes6.is_empty() {
                write!(f, "n6:")?;
                for item in nodes6.iter() {
                    if !first {
                        first = false;
                        write!(f, ",")?;
                    }
                    write!(f, "[{}]", item)?;
                }
            }
        }

        if self.token() != 0 {
            write!(f, ",tok:{}", self.token())?;
        }
        write!(f,
            "}},v:{}",
            version::format_version(self.ver())
        )?;
        Ok(())
    }
}

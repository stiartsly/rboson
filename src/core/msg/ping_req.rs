use std::fmt;
use std::any::Any;
use ciborium::Value as CVal;

use crate::core::{
    version,
    error::{Error, Result},
};

use super::msg::{
    Kind, Method, Msg,
    Data as MsgData
};

pub(crate) struct Message {
    base_data: MsgData
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
                _=> return None,
            }
        }
        Some(())
    }

    fn ser(&self) -> CVal {
        Msg::to_cbor(self)
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl Message {
    pub(crate) fn new() -> Self {
        Self {
            base_data: MsgData::new(
                Kind::Request,
                Method::Ping,
                0
            )
        }
    }
}

impl TryFrom<CVal> for Box<Message> {
    type Error = Error;
    fn try_from(input: CVal) -> Result<Box<Message>> {
        let mut msg = Box::new(Message::new());
        if let None =  msg.from_cbor(&input) {
            return Err(Error::Protocol(
                format!("Invalid cobor value for ping_req message")));
        }
        Ok(msg)
    }
}

impl fmt::Display for Message {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f,
            "y:{},m:{},t:{},v:{}",
            self.kind(),
            self.method(),
            self.txid() as u32,
            version::canonical_version(self.ver())
        )?;
        Ok(())
    }
}

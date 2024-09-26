use std::fmt;
use std::any::Any;
use ciborium::Value as CVal;

use crate::{
    Error,
    error::Result
};
use crate::core::version;
use super::msg::{
    Kind, Method, Msg,
    Data as MsgData
};

pub(crate) struct Message {
    base_data: MsgData,
    msg: String,
    code: i32,
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
                "e" => {
                    let map = v.as_map()?;
                    for (k,v) in map {
                        let k = k.as_text()?;
                        match k {
                            "c" => self.code = v.as_integer()?.try_into().unwrap(),
                            "m" => self.msg = v.as_text()?.to_string(),
                            _ => return None,
                        }
                    }
                }
                _=> return None,
            }
        }
        Some(())
    }

    fn ser(&self) -> CVal {
        let val = CVal::Map(vec![
            (
                CVal::Text(String::from("c")),
                CVal::Integer(self.code.into())
            ),
            (
                CVal::Text(String::from("m")),
                CVal::Text(self.msg.clone()),
            )
        ]);

        let mut root = Msg::to_cbor(self);
        if let Some(map) = root.as_map_mut() {
            map.push((
                CVal::Text(String::from("e")),
                val
            ));
        }
        root
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl Message {
    pub(crate) fn new(method: Method, txid: i32) -> Self {
        Message {
            base_data: MsgData::new(Kind::Error, method, txid),
            code: 0,
            msg: String::from(""),
        }
    }

    pub(crate) fn msg(&self) -> &str {
        &self.msg
    }

    pub(crate) fn code(&self) -> i32 {
        self.code
    }

    pub(crate) fn with_msg(&mut self, str: &str) {
        self.msg = String::from(str);
    }

    pub(crate) fn with_code(&mut self, code: i32) {
        self.code = code
    }
}

impl TryFrom<CVal> for Box<Message> {
    type Error = Error;
    fn try_from(input: CVal) -> Result<Box<Message>> {
        let mut msg = Self::new(Message::new(Method::Unknown, 0));
        if let None =  msg.from_cbor(&input) {
            return Err(Error::Protocol(
                format!("Invalid cobor value for error message")));
        }
        Ok(msg)
    }
}

impl fmt::Display for Message {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f,
            "y:{},m:{},t:{},e:{{c:{}.m:{}}}v:{}",
            self.kind(),
            self.method(),
            self.txid(),
            self.code(),
            self.msg(),
            version::formatted_version(self.ver())
        )?;
        Ok(())
    }
}

use std::rc::Rc;
use ciborium::Value as CVal;

use crate::NodeInfo;

pub(crate) struct Data {
    nodes4  : Option<Vec<Rc<NodeInfo>>>,
    nodes6  : Option<Vec<Rc<NodeInfo>>>,
    token   : i32,
}

impl Data {
    pub(crate) fn new() -> Self {
        Self {
            nodes4  : None,
            nodes6  : None,
            token   : 0,
        }
    }
}

pub(crate) trait Msg {
    fn data(&self) -> &Data;
    fn data_mut(&mut self) -> &mut Data;

    fn nodes4(&self) -> Option<&[Rc<NodeInfo>]> {
        self.data().nodes4.as_deref()
    }

    fn nodes6(&self) -> Option<&[Rc<NodeInfo>]> {
        self.data().nodes6.as_deref()
    }

    fn token(&self) -> i32 {
        self.data().token
    }

    fn populate_closest_nodes4(&mut self, nodes: Vec<Rc<NodeInfo>>) {
        self.data_mut().nodes4 = Some(nodes)
    }

    fn populate_closest_nodes6(&mut self, nodes: Vec<Rc<NodeInfo>>) {
        self.data_mut().nodes6 = Some(nodes)
    }

    fn populate_token(&mut self, token: i32) {
        self.data_mut().token = token
    }

    fn to_cbor(&self) -> CVal {
        let nodes4 = self.nodes4().map_or(vec![], |ns| {
            ns.iter().map(|v| v.to_cbor()).collect()
        });

        let nodes6 = self.nodes6().map_or(vec![], |ns| {
            ns.iter().map(|v| v.to_cbor()).collect()
        });

        let mut vec = Vec::new();
        if !nodes4.is_empty() {
            vec.push((
                CVal::Text(String::from("n4")),
                CVal::Array(nodes4)
            ));
        }
        if !nodes6.is_empty() {
            vec.push((
                CVal::Text(String::from("n6")),
                CVal::Array(nodes6)
            ));
        }
        vec.push((
            CVal::Text(String::from("tok")),
            CVal::Integer(self.token().into())
        ));

        CVal::Map(vec)
    }
}

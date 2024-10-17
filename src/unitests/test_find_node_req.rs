use std::rc::Rc;
use crate::Id;
use crate::core::msg::{
    Msg,
    lookup_req::Msg as LookupMsg,
    find_node_req::Message,
};

#[test]
fn test_cbor() {
    let nodeid = Rc::new(Id::random());
    let mut msg = Message::new();
    msg.with_target(nodeid.clone());
    msg.with_want4(true);

    let cval = msg.ser();
    let mut decoded_msg = Message::new();
    let result = decoded_msg.from_cbor(&cval);
    assert_eq!(result.is_some(), true);
    assert_eq!(decoded_msg.target(), msg.target());
    assert_eq!(decoded_msg.target(), nodeid);
    assert_eq!(decoded_msg.want4(), true);
    assert_eq!(decoded_msg.want6(), false);
    assert_eq!(decoded_msg.want_token(), true);
}

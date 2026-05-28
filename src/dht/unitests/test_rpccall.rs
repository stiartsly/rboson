use std::{
    net::SocketAddr,
    sync::{Arc, Mutex},
};

use crate::{Id, NodeInfo};
use crate::dht::{
    msg::msg::{Message, Method},
    routing::kbucket_entry::KBucketEntry,
    rpc::{
        rpccall::{RpcCall, State},
        rpc_target::Reachability,
    },
};

fn make_target(addr: &str) -> NodeInfo {
    NodeInfo::new(
        Id::random(),
        addr.parse::<SocketAddr>().unwrap()
    )
}

fn make_kentry(addr: &str) -> KBucketEntry {
    let target = make_target(addr);
    KBucketEntry::new(
        target.id().clone(),
        *target.socket_addr()
    )
}

fn make_matching_response(call: &RpcCall) -> Message {
    let mut rsp = Message::ping_rsp(call.txid());
    rsp.set_id(call.target_id());
    rsp.set_remote(call.target_id(), call.target().socket_addr());
    rsp
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_expected_rtt() {
        let target = make_target("127.0.0.1:40001");
        let req = Message::ping_req();
        let expected_txid = req.txid();
        let mut call = RpcCall::with_node(target.clone(), req);

        let expected_id = Id::random();
        call.set_localid(expected_id);
        call.set_expected_rtt_if_absent(40);

        assert_eq!(call.req().id(), &expected_id);
        assert_eq!(call.txid(), expected_txid);
        assert_eq!(call.target_id(), target.id().clone());
        assert_eq!(call.target().socket_addr(), target.socket_addr().clone());
        assert_eq!(call.is_reachable_at_creation(), false);
        assert_eq!(call.is_expected_rtt_set(), true);
        assert_eq!(call.expected_rtt(), 40);

        call.set_expected_rtt(90);
        assert_eq!(call.expected_rtt(), 90);
        assert_eq!(call.state(), State::Unsent);
        assert_eq!(call.is_pending(), true);
        assert_eq!(call.cause().is_none(), true);
    }

    #[test]
    fn test_response_and_update_state() {
        let mut entry = make_kentry("127.0.0.1:40002");
        entry.set_reachable(true);

        let req = Message::ping_req();
        let call = Arc::new(Mutex::new(RpcCall::with_kentry(entry, req)));
        call.lock().unwrap().set_cloned(call.clone());
        call.lock().unwrap().set_expected_rtt(50);

        let state_changes = Arc::new(Mutex::new(Vec::new()));
        let state_changes_cb = state_changes.clone();
        call.lock().unwrap().set_state_changed_cb(move |_, _, current| {
            state_changes_cb.lock().unwrap().push(current);
        });

        let responded = Arc::new(Mutex::new(0usize));
        let responded_cb = responded.clone();
        //call.lock().unwrap().set_responsed_cb(Box::new(move |_, _| {
        //    *responded_cb.lock().unwrap() += 1;
        //}));

        call.lock().unwrap().sent();
        let rsp = make_matching_response(&call.lock().unwrap());
        call.lock().unwrap().respond(rsp);

        let locked = call.lock().unwrap();
        assert_eq!(locked.is_reachable_at_creation(), true);
        assert_eq!(locked.state(), State::Responded);
        assert_eq!(locked.rsp().is_some(), true);
        assert_eq!(locked.id_mismatched(), false);
        assert_eq!(locked.addr_mismatched(), false);
        assert_eq!(locked.rtt().is_some(), true);
        assert_eq!(*responded.lock().unwrap(), 1);
        assert_eq!(*state_changes.lock().unwrap(), vec![State::Sent, State::Responded]);
        drop(locked);

        let target = make_target("127.0.0.1:40003");
        let req = Message::ping_req();
        let error_call = Arc::new(Mutex::new(RpcCall::with_node(target.clone(), req)));
        error_call.lock().unwrap().set_cloned(error_call.clone());

        let mut err = Message::error(Method::Ping, error_call.lock().unwrap().txid(), 500, "boom".into());
        err.set_id(target.id().clone());
        err.set_remote(target.id().clone(), *target.socket_addr());
        error_call.lock().unwrap().respond(err);

        let locked = error_call.lock().unwrap();
        assert_eq!(locked.state(), State::Err);
        assert_eq!(locked.cause().unwrap().to_string().contains("boom"), true);
    }

    #[test]
    fn test_stall_timeout_fail_cancel() {
        /*
        let target = make_target("127.0.0.1:40004");
        let req = Arc::new(Mutex::new(Message::ping_req()));
        let call = Arc::new(Mutex::new(RpcCall::with_node(target.clone(), req)));
        call.lock().unwrap().set_cloned(call.clone());
        call.lock().unwrap().set_expected_rtt(25);

        let stalled = Arc::new(Mutex::new(0usize));
        let stalled_cb = stalled.clone();
        call.lock().unwrap().set_stalled_cb(Box::new(move |_| {
            *stalled_cb.lock().unwrap() += 1;
        }));

        let timed_out = Arc::new(Mutex::new(0usize));
        let timed_out_cb = timed_out.clone();
        call.lock().unwrap().set_timeout_cb(Box::new(move |_| {
            *timed_out_cb.lock().unwrap() += 1;
        }));

        call.lock().unwrap().sent();
        let inconsistent = make_matching_response(&call.lock().unwrap());
        call.lock().unwrap().respond_inconsistent_socket(inconsistent);
        assert_eq!(call.lock().unwrap().state(), State::Stalled);
        assert_eq!(*stalled.lock().unwrap(), 1);

        call.lock().unwrap().update_state(State::Timeout);
        assert_eq!(call.lock().unwrap().state(), State::Timeout);
        assert_eq!(*timed_out.lock().unwrap(), 1);

        let target = make_target("127.0.0.1:40005");
        let req = Arc::new(Mutex::new(Message::ping_req()));
        let mut wrong_method = RpcCall::with_node(target.clone(), req);
        let mut rsp = Message::store_value_rsp(wrong_method.txid());
        rsp.set_id(target.id().clone());
        rsp.set_remote(target.id().clone(), *target.socket_addr());
        wrong_method.respond_wrong_method(Arc::new(Mutex::new(rsp)));
        assert_eq!(wrong_method.state(), State::Err);
        assert_eq!(wrong_method.cause().unwrap().to_string().contains("wrong method"), true);

        let req = Arc::new(Mutex::new(Message::ping_req()));
        let mut failed = RpcCall::with_node(target.clone(), req);
        failed.fail(ProtocolError::new("failed".into()));
        assert_eq!(failed.state(), State::Err);
        assert_eq!(failed.cause().unwrap().to_string().contains("failed"), true);

        let req = Arc::new(Mutex::new(Message::ping_req()));
        let mut canceled = RpcCall::with_node(target, req);
        canceled.cancel();
        assert_eq!(canceled.state(), State::Canceled);
        assert_eq!(canceled.is_pending(), false);
        */
    }

    #[tokio::test]
    async fn test_scheduler_timeout_canceled() {
        /*
        let target = make_target("127.0.0.1:40006");
        let req = Arc::new(Mutex::new(Message::ping_req()));
        let call = Arc::new(Mutex::new(RpcCall::with_node(target, req)));
        call.lock().unwrap().set_cloned(call.clone());

        let timed_out = Arc::new(Mutex::new(0usize));
        let timed_out_cb = timed_out.clone();
        call.lock().unwrap().set_timeout_cb(Box::new(move |_| {
            *timed_out_cb.lock().unwrap() += 1;
        }));

        call.lock().unwrap().sent();
        let rsp = make_matching_response(&call.lock().unwrap());
        call.lock().unwrap().respond(rsp);

        sleep(Duration::from_millis(60)).await;

        assert_eq!(call.lock().unwrap().state(), State::Responded);
        assert_eq!(*timed_out.lock().unwrap(), 0);
        */
    }
}

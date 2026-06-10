use std::{
    net::SocketAddr,
    sync::{Arc, Mutex},
};

use crate::{Id, NodeInfo};
use crate::errors::ProtocolError;
use crate::dht::{
    msg::{msg, msg::Method},
    routing::KBucketEntry,
    rpc::{
        RpcCall, rpccall::State,
        Reachability,
        Listener
    }
};

fn make_nodeinfo(addr: &str) -> NodeInfo {
    NodeInfo::new(
        Id::random(),
        addr.parse::<SocketAddr>().unwrap()
    )
}

fn make_entry(addr: &str) -> KBucketEntry {
    let target = make_nodeinfo(addr);
    let mut entry = KBucketEntry::new(
        target.id().clone(),
        *target.socket_addr()
    );
    entry.set_reachable(true);
    entry
}

fn make_response(call: &RpcCall) -> msg::Message {
    let mut rsp = msg::ping_response(call.txid());
    rsp.set_nodeid(call.target_id());
    rsp.set_remote(call.target_id(), call.target().socket_addr());
    rsp
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_with_node() {
        let target = make_nodeinfo("127.0.0.1:40001");
        let local_nodeid = Id::random();
        let mut req = msg::ping_request();
        req.set_nodeid(local_nodeid.clone());

        let txid = req.txid();
        let call = RpcCall::new(target.clone(), req);

        //call.set_local_id(local_nodeid);
        // call.set_expected_rtt_if_absent(40);

        assert_eq!(call.req().nodeid(), &local_nodeid);
        assert_eq!(call.txid(), txid);
        assert_eq!(call.target_id(), target.id().clone());
        assert_eq!(call.is_reachable_at_creation(), false);
        // assert_eq!(call.expected_rtt(), 40);
        assert_eq!(call.state(), State::Unsent);

        // call.set_expected_rtt(90);
        // assert_eq!(call.expected_rtt(), 90);
        assert_eq!(call.state(), State::Unsent);
        assert_eq!(call.is_pending(), true);
        assert_eq!(call.cause().is_none(), true);
    }

    #[test]
    fn test_with_entry() {
        let target = make_entry("127.0.0.1:40002");
        let local_nodeid = Id::random();
        let mut req = msg::ping_request();
        req.set_nodeid(local_nodeid.clone());
        let txid = req.txid();
        let call = RpcCall::new(target.clone(), req);

        //  call.set_local_id(local_nodeid);
        //  call.set_expected_rtt_if_absent(42);

        assert_eq!(call.req().nodeid(), &local_nodeid);
        assert_eq!(call.txid(), txid);
        assert_eq!(call.target_id(), target.id().clone());
        assert_eq!(call.is_reachable_at_creation(), true);
       // assert_eq!(call.expected_rtt(), 42);
        assert_eq!(call.state(), State::Unsent);

      //  call.set_expected_rtt(91);
      //  assert_eq!(call.expected_rtt(), 91);
        assert_eq!(call.state(), State::Unsent);
        assert_eq!(call.is_pending(), true);
        assert_eq!(call.cause().is_none(), true);
    }

    #[test]
    fn test_update_state() {
        let mut entry = make_entry("127.0.0.1:40002");
        entry.set_reachable(true);

        let req = msg::ping_request();
        let call = RpcCall::new(entry.clone(), req);
        let call = Arc::new(Mutex::new(call));
       // call.lock().unwrap().set_expected_rtt(50);

        let state_changes = Arc::new(Mutex::new(State::Unsent));
        let responded = Arc::new(Mutex::new(0usize));
        let mut listener = Listener::new({
            let state_changes_cb = state_changes.clone();
            move |_, _, cur| {
                *state_changes_cb.lock().unwrap() = cur;
        }});

        listener.response_fn({
            let state_changes_cb = state_changes.clone();
            let responded_cb = responded.clone();
            move |_| {
                *state_changes_cb.lock().unwrap() = State::Responded;
                *responded_cb.lock().unwrap() += 1;
        }});
        listener.stall_fn({
            let state_changes_cb = state_changes.clone();
            move|_| {
                *state_changes_cb.lock().unwrap() = State::Stalled;
        }});
        listener.timeout_fn({
            let state_changes_cb = state_changes.clone();
            move |_| {
                *state_changes_cb.lock().unwrap() = State::Timeout;
        }});
        call.lock().unwrap().set_listener(listener);

        call.lock().unwrap().sent();
        assert_eq!(call.lock().unwrap().state(), State::Sent);
        assert_eq!(state_changes.lock().unwrap().to_owned(), State::Sent);

        let rsp = make_response(&call.lock().unwrap());
        call.lock().unwrap().respond(&rsp);
        assert_eq!(call.lock().unwrap().state(), State::Responded);
        assert_eq!(responded.lock().unwrap().to_owned(), 1);
        assert_eq!(state_changes.lock().unwrap().to_owned(), State::Responded);

        let locked = call.lock().unwrap();
        assert_eq!(locked.is_reachable_at_creation(), true);
        assert_eq!(locked.state(), State::Responded);
        assert_eq!(locked.rsp().is_none(), true);
        assert_eq!(locked.nodeid_mismatched(), true);
        assert_eq!(locked.addr_mismatched(), true);
        // assert_eq!(locked.rtt().is_some(), true);
        assert_eq!(*responded.lock().unwrap(), 1);
        assert_eq!(*state_changes.lock().unwrap(), State::Responded);
        drop(locked);

        // test error response
        let target = make_nodeinfo("127.0.0.1:40003");
        let req = msg::ping_request();
        let error_call = Arc::new(Mutex::new(RpcCall::new(target.clone(), req)));
       // error_call.lock().unwrap().set_cloned(error_call.clone());

        let mut err = msg::error_msg(Method::Ping, error_call.lock().unwrap().txid(), 500, "boom".into());
        err.set_nodeid(target.id().clone());
        err.set_remote(target.id().clone(), *target.socket_addr());
        error_call.lock().unwrap().respond(&err);

        let locked = error_call.lock().unwrap();
        assert_eq!(locked.state(), State::Err);
        assert_eq!(locked.cause().unwrap().to_string().contains("boom"), true);
    }

    #[test]
    fn test_stall_timeout_cancel() {
        let target = make_nodeinfo("127.0.0.1:40004");
        let req = msg::ping_request();
        let call = Arc::new(Mutex::new(RpcCall::new(target.clone(), req)));
       // call.lock().unwrap().set_expected_rtt(25);

        let stalled = Arc::new(Mutex::new(0usize));
        let timed_out = Arc::new(Mutex::new(0usize));
        let mut listener = Listener::new(|_, _, _| {});
        listener.stall_fn({
            let stalled_cb = stalled.clone();
            move |_| { *stalled_cb.lock().unwrap() += 1; }
        });
        listener.timeout_fn({
            let timed_out_cb = timed_out.clone();
            move |_| { *timed_out_cb.lock().unwrap() += 1; }
        });
        call.lock().unwrap().set_listener(listener);

        call.lock().unwrap().sent();
        let inconsistent = make_response(&call.lock().unwrap());
        call.lock().unwrap().respond_inconsistent_socket(inconsistent);
        assert_eq!(call.lock().unwrap().state(), State::Stalled);
        assert_eq!(*stalled.lock().unwrap(), 1);

        call.lock().unwrap().update_state(State::Timeout);
        assert_eq!(call.lock().unwrap().state(), State::Timeout);
        assert_eq!(*timed_out.lock().unwrap(), 1);

        let target2 = make_nodeinfo("127.0.0.1:40005");
        let req2 = msg::ping_request();
        let mut wrong_method = RpcCall::new(target2.clone(), req2);
        let mut rsp = msg::store_value_response(wrong_method.txid());
        rsp.set_nodeid(target2.id().clone());
        rsp.set_remote(target2.id().clone(), *target2.socket_addr());
        wrong_method.respond_wrong_method(rsp);
        assert_eq!(wrong_method.state(), State::Err);
        assert_eq!(wrong_method.cause().unwrap().to_string().contains("wrong method"), true);

        let req3 = msg::ping_request();
        let mut failed = RpcCall::new(target2.clone(), req3);
        let err = ProtocolError::new("failed") as Box<dyn std::error::Error + Send + Sync>;
        failed.fail(&err);
        assert_eq!(failed.state(), State::Err);
        assert_eq!(failed.cause().is_none(), true);

        let req4 = msg::ping_request();
        let mut canceled = RpcCall::new(target2, req4);
        canceled.cancel();
        assert_eq!(canceled.state(), State::Canceled);
        assert_eq!(canceled.is_pending(), false);
    }

    #[ignore]
    #[tokio::test]
    async fn test_scheduler_timeout_canceled() {
        /*
        let target = make_target("127.0.0.1:40006");
        let req = Arc::new(Mutex::new(msg::ping_request()));
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

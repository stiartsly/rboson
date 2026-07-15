use std::{
    cell::RefCell,
    net::SocketAddr,
    rc::Rc,
};
use tokio::sync::mpsc;

use crate::{Id, NodeInfo};
use crate::dht::{
    msg::{msg, msg::Method},
    routing::KBucketEntry,
    rpc::{
        RpcCall, rpccall::State,
        Reachability,
        Listener
    },
    timer_client::{LocalTimerClient, LocalTimerCmd},
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

fn finalize_call(call: &mut RpcCall) {
    let req = Rc::new(call.take_transient());
    call.set_request(req);
}

fn make_timer_client() -> Rc<LocalTimerClient> {
    let (tx, _rx) = mpsc::unbounded_channel::<LocalTimerCmd>();
    Rc::new(LocalTimerClient::new(tx))
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
        let mut call = RpcCall::new(target.clone(), req);
        finalize_call(&mut call);

        assert_eq!(call.req().nodeid(), &local_nodeid);
        assert_eq!(call.txid(), txid);
        assert_eq!(call.target_id(), target.id().clone());
        assert_eq!(call.state(), State::Unsent);
        assert_eq!(call.state(), State::Unsent);
    }

    #[test]
    fn test_with_entry() {
        let target = make_entry("127.0.0.1:40002");
        let local_nodeid = Id::random();
        let mut req = msg::ping_request();
        req.set_nodeid(local_nodeid.clone());
        let txid = req.txid();
        let mut call = RpcCall::new(target.clone(), req);
        finalize_call(&mut call);

        assert_eq!(call.req().nodeid(), &local_nodeid);
        assert_eq!(call.txid(), txid);
        assert_eq!(call.target_id(), target.id().clone());
        assert_eq!(call.state(), State::Unsent);
        assert_eq!(call.state(), State::Unsent);
    }

    #[test]
    fn test_update_state() {
        let req = msg::ping_request();
        let mut call = RpcCall::new(make_entry("127.0.0.1:40002"), req);
        finalize_call(&mut call);
        let call = Rc::new(RefCell::new(call));
        call.borrow_mut().set_cloned(Rc::downgrade(&call));
        let timer_client = make_timer_client();

        let state_changes = Rc::new(RefCell::new(State::Unsent));
        let responded = Rc::new(RefCell::new(0usize));
        let mut listener = Listener::new({
            let state_changes_cb = state_changes.clone();
            move |_, _, cur| {
                *state_changes_cb.borrow_mut() = cur;
        }});

        listener.response_fn({
            let state_changes_cb = state_changes.clone();
            let responded_cb = responded.clone();
            move |_| {
                *state_changes_cb.borrow_mut() = State::Responded;
                *responded_cb.borrow_mut() += 1;
        }});
        listener.stall_fn({
            let state_changes_cb = state_changes.clone();
            move|_| {
                *state_changes_cb.borrow_mut() = State::Stalled;
        }});
        listener.timeout_fn({
            let state_changes_cb = state_changes.clone();
            move |_| {
                *state_changes_cb.borrow_mut() = State::Timeout;
        }});
        call.borrow_mut().set_listener(listener);

        call.borrow_mut().sent(timer_client);
        assert_eq!(call.borrow().state(), State::Sent);
        assert_eq!(*state_changes.borrow(), State::Sent);

        let rsp = Rc::new(make_response(&call.borrow()));
        call.borrow_mut().respond(rsp);
        assert_eq!(call.borrow().state(), State::Responded);
        assert_eq!(*responded.borrow(), 1);
        assert_eq!(*state_changes.borrow(), State::Responded);

        let locked = call.borrow();
        assert_eq!(locked.state(), State::Responded);
        assert!(locked.rsp().is_some());
        assert_eq!(locked.nodeid_mismatched(), false);
        assert_eq!(locked.addr_mismatched(), false);
        assert_eq!(*responded.borrow(), 1);
        assert_eq!(*state_changes.borrow(), State::Responded);
        drop(locked);

        let target = make_nodeinfo("127.0.0.1:40003");
        let req = msg::ping_request();
        let mut error_call = RpcCall::new(target.clone(), req);
        finalize_call(&mut error_call);

        let mut err = msg::error_msg(Method::Ping, error_call.txid(), 500, "boom".into());
        err.set_nodeid(target.id().clone());
        err.set_remote(target.id().clone(), *target.socket_addr());
        error_call.respond(Rc::new(err));
        assert_eq!(error_call.state(), State::Err);
    }

    #[test]
    fn test_stall_timeout_cancel() {
        let target = make_nodeinfo("127.0.0.1:40004");
        let req = msg::ping_request();
        let mut call = RpcCall::new(target.clone(), req);
        finalize_call(&mut call);
        let call = Rc::new(RefCell::new(call));
        call.borrow_mut().set_cloned(Rc::downgrade(&call));
        let timer_client = make_timer_client();

        let stalled = Rc::new(RefCell::new(0usize));
        let timed_out = Rc::new(RefCell::new(0usize));
        let mut listener = Listener::new(|_, _, _| {});
        listener.stall_fn({
            let stalled_cb = stalled.clone();
            move |_| { *stalled_cb.borrow_mut() += 1; }
        });
        listener.timeout_fn({
            let timed_out_cb = timed_out.clone();
            move |_| { *timed_out_cb.borrow_mut() += 1; }
        });
        call.borrow_mut().set_listener(listener);

        call.borrow_mut().sent(timer_client);
        call.borrow_mut().respond_inconsistent_socket();
        assert_eq!(call.borrow().state(), State::Stalled);
        assert_eq!(*stalled.borrow(), 1);

        call.borrow_mut().update_state(State::Timeout);
        assert_eq!(call.borrow().state(), State::Timeout);
        assert_eq!(*timed_out.borrow(), 1);

        let target2 = make_nodeinfo("127.0.0.1:40005");
        let req2 = msg::ping_request();
        let mut wrong_method = RpcCall::new(target2.clone(), req2);
        finalize_call(&mut wrong_method);
        wrong_method.respond_wrong_method();
        assert_eq!(wrong_method.state(), State::Err);

        let req3 = msg::ping_request();
        let mut failed = RpcCall::new(target2.clone(), req3);
        finalize_call(&mut failed);
        failed.fail();
        assert_eq!(failed.state(), State::Err);
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

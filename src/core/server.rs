use std::rc::Rc;
use std::cell::RefCell;
use std::time::SystemTime;
use std::sync::{Arc, Mutex};
use std::collections::{HashMap, LinkedList};

use log::{warn, error};
use tokio::{
    io,
    runtime,
    net::UdpSocket,
    time::{sleep, interval_at, Duration}
};

use crate::{
    as_millis,
    id, Id,
    Error
};

use crate::core::{
    constants,
    version,
    dht::DHT,
    rpccall::RpcCall,
    node_runner::NodeRunner,
    scheduler::{self, Scheduler},
    msg::msg::{self, Msg},
    msg::{deser, serialize},
};

pub(crate) struct Server<> {
    nodeid: Rc<Id>,
    // started: SystemTime,

    recv_msg_num: i32,
    recv_msg_num_at_last_reachable_check: i32,

    reachable: bool,
    last_reachable_check: SystemTime,

    calls: Rc<RefCell<HashMap<i32, Rc<RefCell<RpcCall>>>>>,

    queue4: Option<Rc<RefCell<LinkedList<Rc<RefCell<Box<dyn Msg>>>>>>>,
    scheduler:  Rc<RefCell<Scheduler>>,

}

impl Server {
    pub fn new(nodeid: Rc<Id>) -> Self {
        Self {
            nodeid,
            // started: SystemTime::UNIX_EPOCH,

            recv_msg_num: 0,
            recv_msg_num_at_last_reachable_check: 0,

            reachable: false,
            last_reachable_check: SystemTime::UNIX_EPOCH,

            calls: Rc::new(RefCell::new(HashMap::new())),
            queue4: Some(Rc::new(RefCell::new(LinkedList::new()))),

            scheduler: Rc::new(RefCell::new(Scheduler::new())),
        }
    }

    pub(crate) fn scheduler(&self) -> Rc<RefCell<Scheduler>> {
        self.scheduler.clone()
    }

    pub(crate) fn nodeid(&self) -> &Id {
        &self.nodeid
    }

    pub(crate) fn number_of_acitve_calls(&self) -> usize {
        self.calls.borrow().len()
    }

    pub(crate) fn start(&mut self) {}

    pub(crate) fn stop(&mut self) {
        self.scheduler().borrow_mut()
            .cancel();
    }

    pub(crate) fn is_reachable(&self) -> bool {
        self.reachable
    }

    fn recv_msg_num_incr(&mut self) {
        self.recv_msg_num += 1;
    }

    pub(crate) fn update_reachability(&mut self) {
        // Avoid pinging too frequently if we're not receiving any response
        // (the connection might be dead)
        if self.recv_msg_num != self.recv_msg_num_at_last_reachable_check {
            self.reachable = true;
            self.last_reachable_check = SystemTime::now();
            self.recv_msg_num_at_last_reachable_check = self.recv_msg_num;
            return;
        }

        if as_millis!(self.last_reachable_check) >  constants::RPC_SERVER_REACHABILITY_TIMEOUT {
            self.reachable = false;
        }
    }

    pub(crate) fn send_msg(&mut self, msg: Rc<RefCell<Box<dyn Msg>>>) {
        msg.borrow_mut().set_id(self.nodeid());
        self.queue4.as_ref().unwrap().borrow_mut().push_back(msg);
    }

    pub(crate) fn send_call(&mut self, call: Rc<RefCell<RpcCall>>) {
        let txid  = call.borrow().txid();
        self.calls.borrow_mut().insert(txid, call.clone());

        let calls_cloned = self.calls.clone();
        call.borrow_mut().set_responsed_fn(|_,_| {});
        call.borrow_mut().set_stalled_fn(|_| {});
        call.borrow_mut().set_timeout_fn(move |c| {
            if calls_cloned.borrow_mut().remove(&txid).is_some() {
                c.dht().borrow_mut().on_timeout(c);
            };
        });

        if let Some(msg) = call.borrow().req() {
            msg.borrow_mut().set_txid(txid);
            msg.borrow_mut().with_associated_call(call.clone());
            self.send_msg(msg);
        };
    }
}

pub(crate) fn run_loop(
    runner: Rc<RefCell<NodeRunner>>,
    server: Rc<RefCell<Server>>,
    dht4: Option<Rc<RefCell<DHT>>>,
    dht6: Option<Rc<RefCell<DHT>>>,
    quit: Arc<Mutex<bool>>) -> io::Result<()>
{
    let rt = runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();

    rt.block_on(async move {
        let mut sock4 = None;
        let mut buff4 = None;
        let mut queu4 = None;

        if let Some(dht) = dht4.as_ref() {
            sock4 = Some(UdpSocket::bind(dht.borrow().addr()).await?);
            buff4 = Some(Rc::new(RefCell::new(vec![0u8; 1024])));
            queu4 = server.borrow_mut().queue4.clone();
        }

        let mut sock6 = None;
        let mut buff6 = None;
        let mut queu6 = None;

        if let Some(dht) = dht6.as_ref() {
            sock6 = Some(UdpSocket::bind(dht.borrow().addr()).await?);
            buff6 = Some(Rc::new(RefCell::new(vec![0u8; 1024])));
            queu6 = None;
        }

        let mut interval = interval_at(
            server.borrow().scheduler.borrow().next_timeout(),
            Duration::from_secs(60*60)
        );

        let mut running = true;
        while running {
            tokio::select! {
                res = read_socket(sock4.as_ref(), buff4.as_ref(), server.clone(), |id, encrypted| {
                    runner.borrow().decrypt_into(id, encrypted)
                }), if sock4.is_some() => {
                    match res {
                        Ok(data) => {
                            if let Some(msg) = data {
                                dht4.as_ref().unwrap().borrow_mut().on_message(msg)
                            }
                        },
                        Err(_) => {},
                    }
                }

                res = read_socket(sock6.as_ref(), buff6.as_ref(), server.clone(), |id, encrypted| {
                    runner.borrow().decrypt_into(id, encrypted)
                }), if sock6.is_some() => {
                    match res {
                        Ok(data) => {
                            if let Some(msg) = data {
                                dht6.as_ref().unwrap().borrow_mut().on_message(msg)
                            }
                        },
                        Err(_) => {},
                    }
                }

                res = write_socket(sock4.as_ref(), queu4.as_ref(), dht4.as_ref(), |id, plain| {
                    runner.borrow().encrypt_into(id, plain)
                }), if sock4.is_some() => {
                    match res {
                        Ok(_) => {},
                        Err(_) => {},
                    }
                }

                res = write_socket(sock6.as_ref(), queu6.as_ref(), dht4.as_ref(), |id, plain| {
                    runner.borrow().encrypt_into(id, plain)
                }), if sock6.is_some() => {
                    match res {
                        Ok(_) => {},
                        Err(_) => {},
                    }
                }

                _ = interval.tick() => {
                    let scheduler = server.borrow().scheduler();
                    scheduler::run_jobs(scheduler);
                }
            }

            if *quit.lock().unwrap() {
                running = false;
            }
            if server.borrow().scheduler.borrow().is_updated() {
                interval.reset_at(server.borrow().scheduler.borrow().next_timeout());
            }
        }

        Ok(())
    })
}

async fn read_socket<F>(
    socket: Option<&UdpSocket>,
    buffer: Option<&Rc<RefCell<Vec<u8>>>>,
    server: Rc<RefCell<Server>>,
    mut decrypt: F
) -> Result<Option<Rc<RefCell<Box<dyn Msg>>>>, io::Error>
where F: FnMut(&Id, &mut [u8]) -> Result<Vec<u8>, Error>
{
    let socket = match socket {
        Some(v) => v,
        None => return Ok(None),
    };

    let buf = match buffer {
        Some(v) => v,
        None => return Ok(None),
    };

    let mut buf = buf.borrow_mut();
    let (len, from) = socket.recv_from(&mut buf).await?;
    let from_id = Id::try_from(&buf[0.. id::ID_BYTES]).unwrap();

    let plain = match decrypt(&from_id, &mut buf[id::ID_BYTES .. len]) {
        Ok(v) => v,
        Err(err) => {
            warn!("Decrypt packet from {} error {}, ignored it", err, from);
            return Ok(None);
        }
    };

    let msg = match deser(&plain) {
        Ok(msg) => msg,
        Err(err) => {
            warn!("Got a wrong packet from {} with {}, ignored it", from, err);
            return Ok(None);
        }
    };

    server.borrow_mut().recv_msg_num_incr();

    msg.borrow_mut().set_id(&from_id);
    msg.borrow_mut().set_origin(&from);

    #[cfg(debug_assertions)]
    {
        use log::info;
        info!("Received message: {}/{} from {}:[size: {}] - {}",
            msg.borrow().method(),
            msg.borrow().kind(),
            from,
            len,
            msg.borrow());
    }

    // txid should not be zero if it's not Error message.
    if msg.borrow().kind() != msg::Kind::Error && msg.borrow().txid() == 0 {
        warn!("Received a message with invalid txid, discarded it");
        return Ok(None);
    }

    // Just respond to incoming request, no need to match them to pending requests
    if msg.borrow().kind() == msg::Kind::Request {
        return Ok(Some(msg));
    }

    // Check if it's a response to an outstanding request.
    let calls = server.borrow().calls.clone();
    let call = calls.borrow_mut().remove(&msg.borrow().txid());

    if let Some(call) = call {
        let req = match call.borrow().req() {
            Some(v) => v.clone(),
            None => return Ok(None)
        };

        if req.borrow().remote_addr() == msg.borrow().origin() {
            msg.borrow_mut().with_associated_call(call.clone());
            call.borrow_mut().responsed(msg.clone());

            return Ok(Some(msg.clone()));
        }

        // 1. the message is not a request
        // 2. transaction ID matched
        // 3. request destination did not match response source!!
        // this happening by chance is exceedingly unlikely
        // indicates either port-mangling NAT, a multhomed host listening on any-local address
        // or some kind of attack ignore response

        warn!("Transaction id matched, socket address did not, ignoring message, request: {} -> response: {}, version: {}",
            req.borrow().remote_addr(),
            msg.borrow().origin(),
            version::formatted_version(msg.borrow().ver())
        );

        // but expect an upcoming timeout if it's really just a misbehaving node
        call.borrow_mut().responsed_socket_mismatch();
        call.borrow_mut().stall();
        return Ok(None);
    }

    // a) it's not a request
    // b) didn't find a call
    // c) up-time is high enough that it's not a stray from a restart
    // did not expect this response
    if msg.borrow().kind() == msg::Kind::Response { // && as_millis!(self.started) > 2 * 60 * 1000 {
        warn!("Can not find rpc call for response {}", msg.borrow());
        return Ok(None);
    }

    if msg.borrow().kind() == msg::Kind::Error {
        return Ok(Some(msg));
    }

    warn!("Unknown message, gnored it {}.", msg.borrow());
    Ok(None)
}

async fn write_socket<F>(
    socket: Option<&UdpSocket>,
    queue: Option<&Rc<RefCell<LinkedList<Rc<RefCell<Box<dyn Msg>>>>>>>,
    dht: Option<&Rc<RefCell<DHT>>>,
    mut encrypt: F) -> Result<(), io::Error>
where F: FnMut(&Id, &[u8]) -> Result<Vec<u8>, Error>
{
    let socket = match socket {
        Some(v) => v,
        None => return Ok(()),
    };

    let dht = match dht {
        Some(v) => v.clone(),
        None => return Ok(()),
    };

    let queue = match queue {
        Some(v) => v.clone(),
        None => return Ok(()),
    };

    let msg = match queue.borrow_mut().pop_front() {
        Some(v) => v,
        None => {
            sleep(Duration::from_millis(1000)).await;
            return Ok(())
        }
    };

    if let Some(call) = msg.borrow().associated_call() {
        let scheduler = dht.borrow().server().borrow().scheduler();

        dht.borrow_mut().on_send(call.borrow_mut().target_id());
        call.borrow_mut().send(scheduler);
    }

    let plain = serialize(msg.clone());
    let encrypted = match encrypt(msg.borrow().remote_id(), &plain) {
        Ok(v) => v,
        Err(e) => {
            error!("Encrypting packet error {} for message {}", e, msg.borrow());
            return Ok(())
        },
    };

    let mut buf = Vec::new() as Vec<u8>;
    buf.extend_from_slice(msg.borrow().id().as_bytes());
    buf.extend_from_slice(&encrypted);

    match socket.send_to(&buf, msg.borrow().remote_addr()).await {
        Ok(_) => {},
        Err(e) => warn!("Sending message failed {}", e),
    };

    Ok(())
}

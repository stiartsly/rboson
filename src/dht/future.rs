use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll, Waker};
use std::future::Future;

use crate::{
    Id,
    NodeInfo,
    PeerInfo,
    Value,
    JointResult,
    core::Result,
};

use crate::dht::LookupOption;

pub(crate) struct CmdData<T> {
    result: Option<Result<T>>,
    waker : Option<Waker>,
    completed: bool
}

impl<T> CmdData<T> {
    fn new() -> Self {
        Self {
            result: None,
            waker : None,
            completed: false,
        }
    }
}

pub(crate) trait Cmd {
    type CmdResult;

    fn data(&self) -> &CmdData<Self::CmdResult>;
    fn data_mut(&mut self) -> &mut CmdData<Self::CmdResult>;

    fn result(&mut self) -> Result<Self::CmdResult> {
        self.data_mut().result.take().unwrap()
    }

    fn complete(&mut self, result: Result<Self::CmdResult>) {
        if let Some(waker) = self.data_mut().waker.take() {
            self.data_mut().result = Some(result);
            self.data_mut().completed = true;
            waker.wake();
        }
    }

    fn is_completed(&self) -> bool {
        self.data().completed
    }

    fn set_waker(&mut self, waker: Waker) {
        self.data_mut().waker = Some(waker);
        self.data_mut().completed = false;
    }
}

pub(crate) struct FindNodeCmd {
    data: CmdData<JointResult<NodeInfo>>,
    target: Id,
    option: LookupOption,
}

impl FindNodeCmd {
    pub(crate) fn new(target: &Id, option: &LookupOption) -> Self {
        Self {
            data: CmdData::new(),
            target: target.clone(),
            option: option.clone(),
        }
    }

    pub(crate) fn target(&self) -> &Id {
        &self.target
    }

    pub(crate) fn option(&self) -> &LookupOption {
        &self.option
    }
}

impl Cmd for FindNodeCmd {
    type CmdResult = JointResult<NodeInfo>;

    fn data(&self) -> &CmdData<Self::CmdResult> {
        &self.data
    }
    fn data_mut(&mut self) -> &mut CmdData<Self::CmdResult> {
        &mut self.data
    }
}

pub(crate) struct FindValueCmd {
    data: CmdData<Option<Value>>,
    value_id: Id,
    option: LookupOption,
}

impl FindValueCmd {
    pub(crate) fn new(value_id: &Id, option: &LookupOption) -> Self {
        Self {
            data: CmdData::new(),
            value_id: value_id.clone(),
            option: option.clone(),
        }
    }

    pub(crate) fn value_id(&self) -> &Id {
        &self.value_id
    }

    pub(crate) fn option(&self) -> &LookupOption {
        &self.option
    }
}

impl Cmd for FindValueCmd {
    type CmdResult = Option<Value>;

    fn data(&self) -> &CmdData<Self::CmdResult> {
        &self.data
    }
    fn data_mut(&mut self) -> &mut CmdData<Self::CmdResult> {
        &mut self.data
    }
}

pub(crate) struct FindPeerCmd {
    data: CmdData<Vec<PeerInfo>>,
    peer_id: Id,
    expected_num: usize,
    option: LookupOption
}

impl FindPeerCmd {
    pub(crate) fn new(peer_id: &Id, expected_num: usize, option: &LookupOption) -> Self {
        Self {
            data: CmdData::new(),
            peer_id: peer_id.clone(),
            expected_num,
            option: option.clone(),
        }
    }

    pub(crate) fn peer_id(&self) -> &Id {
        &self.peer_id
    }

    pub(crate) fn expected_num(&self) -> usize {
        self.expected_num
    }

    pub(crate) fn option(&self) -> &LookupOption {
        &self.option
    }
}

impl Cmd for FindPeerCmd {
    type CmdResult = Vec<PeerInfo>;

    fn data(&self) -> &CmdData<Self::CmdResult> {
        &self.data
    }
    fn data_mut(&mut self) -> &mut CmdData<Self::CmdResult> {
        &mut self.data
    }
}

pub(crate) struct StoreValueCmd {
    data: CmdData<()>,
    value: Value,
    persistent: bool
}

impl StoreValueCmd {
    pub(crate) fn new(value: &Value, persistent: bool) -> Self {
        Self {
            data: CmdData::new(),
            value: value.clone(),
            persistent: persistent,
        }
    }

    pub(crate) fn value(&self) -> &Value {
        &self.value
    }

    pub(crate) fn persistent(&self) -> bool {
        self.persistent
    }
}

impl Cmd for StoreValueCmd {
    type CmdResult = ();

    fn data(&self) -> &CmdData<Self::CmdResult> {
        &self.data
    }
    fn data_mut(&mut self) -> &mut CmdData<Self::CmdResult> {
        &mut self.data
    }
}

pub(crate) struct AnnouncePeerCmd {
    data: CmdData<()>,
    peer: PeerInfo,
    persistent: bool,
}

impl AnnouncePeerCmd {
    pub(crate) fn new(peer: &PeerInfo, persistent: bool) -> Self {
        Self {
            data: CmdData::new(),
            peer: peer.clone(),
            persistent,
        }
    }

    pub(crate) fn peer(&self) -> &PeerInfo {
        &self.peer
    }

    pub(crate) fn persistent(&self) -> bool {
        self.persistent
    }
}

impl Cmd for AnnouncePeerCmd {
    type CmdResult = ();

    fn data(&self) -> &CmdData<Self::CmdResult> {
        &self.data
    }
    fn data_mut(&mut self) -> &mut CmdData<Self::CmdResult> {
        &mut self.data
    }
}

pub(crate) struct GetValueCmd {
    data: CmdData<Option<Value>>,
    value_id: Id
}

impl GetValueCmd {
    pub(crate) fn new(value_id: &Id) -> Self {
        Self {
            data: CmdData::new(),
            value_id: value_id.clone()
        }
    }

    pub(crate) fn value_id(&self) -> &Id {
        &self.value_id
    }
}

impl Cmd for GetValueCmd {
    type CmdResult = Option<Value>;

    fn data(&self) -> &CmdData<Self::CmdResult> {
        &self.data
    }
    fn data_mut(&mut self) -> &mut CmdData<Self::CmdResult> {
        &mut self.data
    }
}

pub(crate) struct RemoveValueCmd {
    data: CmdData<()>,
    value_id: Id
}

impl RemoveValueCmd {
    pub(crate) fn new(value_id: &Id) -> Self {
        Self {
            data: CmdData::new(),
            value_id: value_id.clone()
        }
    }

    pub(crate) fn value_id(&self) -> &Id {
        &self.value_id
    }
}

impl Cmd for RemoveValueCmd {
    type CmdResult = ();

    fn data(&self) -> &CmdData<Self::CmdResult> {
        &self.data
    }
    fn data_mut(&mut self) -> &mut CmdData<Self::CmdResult> {
        &mut self.data
    }
}

pub(crate) struct GetValueIdsCmd {
    data: CmdData<Vec<Id>>,
}

impl GetValueIdsCmd {
    pub(crate) fn new() -> Self {
        Self {
            data: CmdData::new(),
        }
    }
}

impl Cmd for GetValueIdsCmd {
    type CmdResult = Vec<Id>;

    fn data(&self) -> &CmdData<Self::CmdResult> {
        &self.data
    }
    fn data_mut(&mut self) -> &mut CmdData<Self::CmdResult> {
        &mut self.data
    }
}

pub(crate) struct GetPeerCmd {
    data: CmdData<Option<PeerInfo>>,
    peer_id: Id
}

impl GetPeerCmd {
    pub(crate) fn new(peer_id: &Id) -> Self {
        Self {
            data: CmdData::new(),
            peer_id: peer_id.clone()
        }
    }

    pub(crate) fn peer_id(&self) -> &Id {
        &self.peer_id
    }
}

impl Cmd for GetPeerCmd {
    type CmdResult = Option<PeerInfo>;

    fn data(&self) -> &CmdData<Self::CmdResult> {
        &self.data
    }
    fn data_mut(&mut self) -> &mut CmdData<Self::CmdResult> {
        &mut self.data
    }
}

pub(crate) struct RemovePeerCmd {
    data: CmdData<()>,
    peer_id: Id
}

impl RemovePeerCmd {
    pub(crate) fn new(peer_id: &Id) -> Self {
        Self {
            data: CmdData::new(),
            peer_id: peer_id.clone()
        }
    }

    pub(crate) fn peer_id(&self) -> &Id {
        &self.peer_id
    }
}

impl Cmd for RemovePeerCmd {
    type CmdResult = ();

    fn data(&self) -> &CmdData<Self::CmdResult> {
        &self.data
    }
    fn data_mut(&mut self) -> &mut CmdData<Self::CmdResult> {
        &mut self.data
    }
}

pub(crate) struct GetPeerIdsCmd {
    data: CmdData<Vec<Id>>,
}

impl GetPeerIdsCmd {
    pub(crate) fn new() -> Self {
        Self {
            data: CmdData::new(),
        }
    }
}

impl Cmd for GetPeerIdsCmd {
    type CmdResult = Vec<Id>;

    fn data(&self) -> &CmdData<Self::CmdResult> {
        &self.data
    }
    fn data_mut(&mut self) -> &mut CmdData<Self::CmdResult> {
        &mut self.data
    }
}

#[derive(Clone)]
pub(crate) enum Command {
    FindNode(Arc<Mutex<FindNodeCmd>>),
    FindValue(Arc<Mutex<FindValueCmd>>),
    FindPeer(Arc<Mutex<FindPeerCmd>>),
    StoreValue(Arc<Mutex<StoreValueCmd>>),
    AnnouncePeer(Arc<Mutex<AnnouncePeerCmd>>),
    GetValue(Arc<Mutex<GetValueCmd>>),
    RemoveValue(Arc<Mutex<RemoveValueCmd>>),
    GetValueIds(Arc<Mutex<GetValueIdsCmd>>),
    GetPeer(Arc<Mutex<GetPeerCmd>>),
    RemovePeer(Arc<Mutex<RemovePeerCmd>>),
    GetPeerIds(Arc<Mutex<GetPeerIdsCmd>>),
}

impl Command {
    pub(crate) fn is_completed(&self) -> bool {
        match self {
            Command::FindNode(c)    => c.lock().unwrap().is_completed(),
            Command::FindValue(c)   => c.lock().unwrap().is_completed(),
            Command::FindPeer(c)    => c.lock().unwrap().is_completed(),
            Command::StoreValue(c)  => c.lock().unwrap().is_completed(),
            Command::AnnouncePeer(c)=> c.lock().unwrap().is_completed(),
            Command::GetValue(c)    => c.lock().unwrap().is_completed(),
            Command::RemoveValue(c) => c.lock().unwrap().is_completed(),
            Command::GetValueIds(c) => c.lock().unwrap().is_completed(),
            Command::GetPeer(c)     => c.lock().unwrap().is_completed(),
            Command::RemovePeer(c)  => c.lock().unwrap().is_completed(),
            Command::GetPeerIds(c) => c.lock().unwrap().is_completed(),
        }
    }

    fn set_waker(&mut self, waker: Waker) {
        match self {
            Command::FindNode(s)    => s.lock().unwrap().set_waker(waker),
            Command::FindValue(s)   => s.lock().unwrap().set_waker(waker),
            Command::FindPeer(s)    => s.lock().unwrap().set_waker(waker),
            Command::StoreValue(s)  => s.lock().unwrap().set_waker(waker),
            Command::AnnouncePeer(s)=> s.lock().unwrap().set_waker(waker),
            Command::GetValue(s)    => s.lock().unwrap().set_waker(waker),
            Command::RemoveValue(c) => c.lock().unwrap().set_waker(waker),
            Command::GetValueIds(c) => c.lock().unwrap().set_waker(waker),
            Command::GetPeer(c)     => c.lock().unwrap().set_waker(waker),
            Command::RemovePeer(c)  => c.lock().unwrap().set_waker(waker),
            Command::GetPeerIds(c)  => c.lock().unwrap().set_waker(waker),
        }
    }
}

pub(crate) struct CmdFuture {
    command: Command,
}

impl CmdFuture {
    pub(crate) fn new(command: Command) -> Self {
        Self { command }
    }
}

impl Future for CmdFuture {
    type Output = Result<()>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        if self.command.is_completed() {
            Poll::Ready(Ok(()))
        } else {
            self.command.set_waker(cx.waker().clone());
            Poll::Pending
        }
    }
}

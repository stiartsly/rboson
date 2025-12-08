use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll, Waker};
use std::future::Future;

use crate::{
    core::Result,
};

use crate::messaging::{
    client_device::ClientDevice,
    channel::Channel,
};

#[derive(Default)]
pub(crate) struct Data<T> {
    result  : Option<Result<T>>,
    waker   : Option<Waker>,
    completed: bool
}

impl<T> Data<T> {
    fn new() -> Self {
        Self {
            result: None,
            waker : None,
            completed: false,
        }
    }
}

pub(crate) trait Ack {
    type Value;

    fn data(&self) -> &Data<Self::Value>;
    fn data_mut(&mut self) -> &mut Data<Self::Value>;

    fn result(&mut self) -> Result<Self::Value> {
        self.data_mut().result.take().unwrap()
    }

    fn complete(&mut self, result: Result<Self::Value>) {
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

pub(crate) struct Value<T> {
    data: Data<T>
}

impl<T> Value<T> {
    pub(crate) fn new() -> Self {
        Self {
            data: Data::new()
        }
    }
}

impl<T> Ack for Value<T> {
    type Value = T;

    fn data(&self) -> &Data<Self::Value> {
        &self.data
    }
    fn data_mut(&mut self) -> &mut Data<Self::Value> {
        &mut self.data
    }
}

pub(crate) type DevicesVal  = Value<Vec<ClientDevice>>;
pub(crate) type ChannelVal  = Value<Channel>;
pub(crate) type BoolVal     = Value<()>;
pub(crate) type StringVal   = Value<String>;

#[derive(Clone)]
pub(crate) enum Promise {
    GetDeviceList(Arc<Mutex<DevicesVal>>),
    RevokeDevice(Arc<Mutex<BoolVal>>),
    CreateChannel(Arc<Mutex<ChannelVal>>),
    RemoveChannel(Arc<Mutex<BoolVal>>),
    JoinChannel(Arc<Mutex<BoolVal>>),
    LeaveChannel(Arc<Mutex<BoolVal>>),
    SetChannelOwner(Arc<Mutex<BoolVal>>),
    SetChannelPerm(Arc<Mutex<BoolVal>>),
    SetChannelName(Arc<Mutex<BoolVal>>),
    SetChannelNotice(Arc<Mutex<BoolVal>>),
    SetChannelMemberRole(Arc<Mutex<BoolVal>>),
    BanChannelMembers(Arc<Mutex<BoolVal>>),
    UnbanChannelMembers(Arc<Mutex<BoolVal>>),
    RemoveChannelMembers(Arc<Mutex<BoolVal>>),
    PushContactsUpdate(Arc<Mutex<StringVal>>)
}

impl Promise {
    pub(crate) fn is_completed(&self) -> bool {
        use Promise::*;
        match self {
            RevokeDevice(s) |
            RemoveChannel(s) |
            JoinChannel(s) |
            LeaveChannel(s) |
            SetChannelOwner(s) |
            SetChannelPerm(s) |
            SetChannelName(s)|
            SetChannelNotice(s) |
            SetChannelMemberRole(s) |
            BanChannelMembers(s) |
            UnbanChannelMembers(s) |
            RemoveChannelMembers(s) => s.lock().unwrap().is_completed(),
            GetDeviceList(s)        => s.lock().unwrap().is_completed(),
            CreateChannel(s)        => s.lock().unwrap().is_completed(),
            PushContactsUpdate(s)   => s.lock().unwrap().is_completed(),
        }
    }

    fn set_waker(&mut self, w: Waker) {
        use Promise::*;
        match self {
            RevokeDevice(s) |
            RemoveChannel(s) |
            JoinChannel(s) |
            LeaveChannel(s) |
            SetChannelOwner(s) |
            SetChannelPerm(s) |
            SetChannelName(s)|
            SetChannelNotice(s) |
            SetChannelMemberRole(s) |
            BanChannelMembers(s) |
            UnbanChannelMembers(s) |
            RemoveChannelMembers(s) => s.lock().unwrap().set_waker(w),
            GetDeviceList(s)        => s.lock().unwrap().set_waker(w),
            CreateChannel(s)        => s.lock().unwrap().set_waker(w),
            PushContactsUpdate(s)   => s.lock().unwrap().set_waker(w),
        }
    }
}

pub(crate) struct Waiter {
    promise: Promise,
}

impl Waiter {
    pub(crate) fn new(promise: Promise) -> Self {
        Self { promise }
    }
}

impl Future for Waiter {
    type Output = Result<()>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        if self.promise.is_completed() {
            Poll::Ready(Ok(()))
        } else {
            self.promise.set_waker(cx.waker().clone());
            Poll::Pending
        }
    }
}

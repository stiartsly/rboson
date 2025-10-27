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

#[allow(unused)]
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

pub(crate) struct CommonAck<T> {
    data: Data<T>
}

impl<T> CommonAck<T> {
    pub(crate) fn new() -> Self {
        Self {
            data: Data::new()
        }
    }
}

impl<T> Ack for CommonAck<T> {
    type Value = T;

    fn data(&self) -> &Data<Self::Value> {
        &self.data
    }
    fn data_mut(&mut self) -> &mut Data<Self::Value> {
        &mut self.data
    }
}

pub(crate) type DeviceListAck = CommonAck<Vec<ClientDevice>>;
pub(crate) type RevokeDeviceAck = CommonAck<()>;
pub(crate) type CreateChannelAck = CommonAck<Channel>;
pub(crate) type RemoveChannelAck = CommonAck<()>;
pub(crate) type JoinChannelAck = CommonAck<()>;
pub(crate) type LeaveChannelAck = CommonAck<()>;
pub(crate) type SetChannelOwnerAck = CommonAck<()>;
pub(crate) type SetChannelPermAck = CommonAck<()>;
pub(crate) type SetChannelNameAck = CommonAck<()>;
pub(crate) type SetChannelNoticeAck = CommonAck<()>;
pub(crate) type SetChannelMemberRoleAck = CommonAck<()>;
pub(crate) type BanChannelMembersAck = CommonAck<()>;
pub(crate) type UnbanChannelMembersAck = CommonAck<()>;
pub(crate) type RemoveChannelMembersAck = CommonAck<()>;

#[derive(Clone)]
pub(crate) enum Promise {
    DeviceList(Arc<Mutex<DeviceListAck>>),
    RevokeDevice(Arc<Mutex<RevokeDeviceAck>>),
    CreateChannel(Arc<Mutex<CreateChannelAck>>),
    RemoveChannel(Arc<Mutex<RemoveChannelAck>>),
    JoinChannel(Arc<Mutex<JoinChannelAck>>),
    LeaveChannel(Arc<Mutex<LeaveChannelAck>>),
    SetChannelOwner(Arc<Mutex<SetChannelOwnerAck>>),
    SetChannelPerm(Arc<Mutex<SetChannelPermAck>>),
    SetChannelName(Arc<Mutex<SetChannelNameAck>>),
    SetChannelNotice(Arc<Mutex<SetChannelNoticeAck>>),
    SetChannelMemberRole(Arc<Mutex<SetChannelMemberRoleAck>>),
    BanChannelMembers(Arc<Mutex<BanChannelMembersAck>>),
    UnbanChannelMembers(Arc<Mutex<UnbanChannelMembersAck>>),
    RemoveChannelMembers(Arc<Mutex<RemoveChannelMembersAck>>),
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
            DeviceList(s)           => s.lock().unwrap().is_completed(),
            CreateChannel(s)        => s.lock().unwrap().is_completed(),
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
            DeviceList(s)           => s.lock().unwrap().set_waker(w),
            CreateChannel(s)        => s.lock().unwrap().set_waker(w),
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

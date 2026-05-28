use std::{
    pin::Pin,
    sync::{Arc, Mutex},
    task::{Context, Poll, Waker},
    future::Future
};

use crate::core::errors::Result;

pub(crate) struct Data<T> {
    result: Option<Result<T>>,
    waker : Option<Waker>,
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

    fn result(&mut self) -> Result<T> {
        self.result.take().unwrap()
    }

    fn complete(&mut self, result: Result<T>) {
        if let Some(waker) = self.waker.take() {
            self.result = Some(result);
            self.completed = true;
            waker.wake();
        }
    }

    fn is_completed(&self) -> bool {
        self.completed
    }

    fn set_waker(&mut self, waker: Waker) {
        self.waker = Some(waker);
        self.completed = false;
    }
}

pub(crate) struct PromiseFuture<T>(Arc<Mutex<Data<T>>>);

impl<T> PromiseFuture<T> {
    pub(crate) fn result(&self) -> Result<T> {
        self.0.lock().unwrap().result()
    }
}

impl<T> Unpin for PromiseFuture<T> {}
impl<T> Clone for PromiseFuture<T> {
    fn clone(&self) -> PromiseFuture<T> {
        Self(self.0.clone())
    }
}

impl<T> From<&Promise<T>> for PromiseFuture<T> {
    fn from(promise: &Promise<T>) -> Self {
        PromiseFuture(promise.result.clone())
    }
}

impl<T> Future for PromiseFuture<T> {
    type Output = Result<T>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        if self.0.lock().unwrap().is_completed() {
            return Poll::Ready(self.0.lock().unwrap().result());
        }

        self.0.lock().unwrap().set_waker(cx.waker().clone());
        Poll::Pending
    }
}

pub(crate) struct Promise<T> {
    result: Arc<Mutex<Data<T>>>,
}

impl<T> Promise<T> {
    pub(crate) fn new() -> Self {
        Self {
            result: Arc::new(Mutex::new(Data::<T>::new())),
        }
    }

    pub(crate) fn complete(&self, result: Result<T>) {
        self.result.lock().unwrap().complete(result);
    }

    pub(crate) fn future(&self) -> PromiseFuture<T> {
        PromiseFuture(self.result.clone())
    }
}

use std::{
    future::Future,
    pin::Pin,
    sync::{Arc, Mutex},
    task::{Context, Poll, Waker},
};

use crate::errors::Result;

pub struct Data<T> {
    result: Option<Result<T>>,
    waker: Option<Waker>,
    completed: bool,
}

impl<T> Data<T> {
    fn new() -> Self {
        Self {
            result: None,
            waker: None,
            completed: false,
        }
    }

    fn result(&mut self) -> Result<T> {
        self.result.take().unwrap()
    }

    fn complete(&mut self, result: Result<T>) {
        self.result = Some(result);
        self.completed = true;
        if let Some(waker) = self.waker.take() {
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

pub struct PromiseFuture<T>(Arc<Mutex<Data<T>>>);

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
        let mut data = self.0.lock().unwrap();
        if data.is_completed() {
            Poll::Ready(data.result())
        } else {
            data.set_waker(cx.waker().clone());
            Poll::Pending
        }
    }
}

#[derive(Clone)]
pub struct Promise<T> {
    result: Arc<Mutex<Data<T>>>,
}

impl<T> Promise<T> {
    fn new() -> Self {
        Self {
            result: Arc::new(Mutex::new(Data::<T>::new())),
        }
    }

    fn future(&self) -> PromiseFuture<T> {
        PromiseFuture(self.result.clone())
    }

    pub fn pair() -> (Self, PromiseFuture<T>) {
        let promise = Self::new();
        let future = promise.future();
        (promise, future)
    }

    pub fn complete(&self, result: Result<T>) {
        self.result.lock().unwrap().complete(result);
    }
}

use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll, Waker};
use std::future::Future;

use crate::core::Result;

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

#[derive(Clone)]
pub(crate) struct MyFuture<T>(Arc<Mutex<Data<T>>>);

impl<T> MyFuture<T> {
    pub(crate) fn result(&self) -> Result<T> {
        self.0.lock().unwrap().result()
    }
}

impl<T> Future for MyFuture<T> {
    type Output = Result<T>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        if self.0.lock().unwrap().is_completed() {
            return Poll::Ready(self.0.lock().unwrap().result());
        }

        self.0.lock().unwrap().set_waker(cx.waker().clone());
        Poll::Pending
    }
}

impl Unpin for MyFuture<()> {}

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

    pub(crate) fn is_completed(&self) -> bool {
        self.result.lock().unwrap().is_completed()
    }

    pub(crate) fn future(&self) -> MyFuture<T> {
        MyFuture(self.result.clone())
    }
}

impl<T> From<&Promise<T>> for MyFuture<T> {
    fn from(promise: &Promise<T>) -> Self {
        MyFuture(promise.result.clone())
    }
}

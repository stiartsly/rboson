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
}
pub(crate) trait Method {
    type ResultType;

    fn data(&self) -> &Data<Self::ResultType>;
    fn data_mut(&mut self) -> &mut Data<Self::ResultType>;

    fn result(&mut self) -> Result<Self::ResultType> {
        self.data_mut().result.take().unwrap()
    }

    fn complete(&mut self, result: Result<Self::ResultType>) {
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

pub(crate) struct ResultData<T> {
    data: Data<T>
}

impl<T> ResultData<T> {
    pub(crate) fn new() -> Self {
         Self {
            data: Data::<T>::new()
        }
    }

    pub(crate) fn result(&mut self) -> Result<T> {
        self.data.result()
    }

    pub(crate) fn complete(&mut self, result: Result<T>) {
        self.data.complete(result);
    }

    pub(crate) fn is_completed(&self) -> bool {
        self.data.is_completed()
    }
}

impl<T> Method for ResultData<T> {
    type ResultType = T;

    fn data(&self) -> &Data<Self::ResultType> {
        &self.data
    }
    fn data_mut(&mut self) -> &mut Data<Self::ResultType> {
        &mut self.data
    }
}

pub(crate) struct Promise<T> {
    result: Arc<Mutex<ResultData<T>>>,
}

impl<T> Promise<T> {
    pub(crate) fn new() -> Self {
        Self {
            result: Arc::new(Mutex::new(ResultData::<T>::new())),
        }
    }

    pub(crate) fn result(&self) -> Result<T> {
        self.result.lock().unwrap().result()
    }

    pub(crate) fn waker(&self) -> Arc<Mutex<ResultData<T>>> {
        self.result.clone()
    }
}

impl<T> Future for Promise<T> {
    type Output = Result<()>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        if self.result.lock().unwrap().is_completed() {
            return Poll::Ready(Ok(()))
        }

        self.result.lock().unwrap().set_waker(cx.waker().clone());
        Poll::Pending
    }
}

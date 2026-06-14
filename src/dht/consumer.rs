use std::{
    pin::Pin,
    future::Future,
};

pub(crate) struct Consumer<T> {
    cb: Box<dyn Fn(&T) + Send>,
}

impl<T: Send + 'static> Consumer<T> {
    pub(crate) fn new<F>(cb: F) -> Self
    where
        F: Fn(&T) + 'static + Send
    {
        Self {
            cb: Box::new(cb),
        }
    }

    pub(crate) fn accept(&self, value: &T) {
        (self.cb)(value);
    }
}

type BoxFuture = Pin<Box<dyn Future<Output = ()>>>;
pub(crate) struct AsyncConsumer<T> {
    cb: Box<dyn Fn(T) -> BoxFuture + Send + 'static >
}

impl<T: Send + Sync> AsyncConsumer<T> {
    pub(crate) fn new<F>(handler: F) -> Self
    where
        F: Fn(T) -> BoxFuture + Send + 'static
    {
        Self {cb: Box::new(handler) }
    }

    pub(crate) async fn accept(&self, value: T) {
        (self.cb)(value).await;
    }
}

/*
// Here is a future version of AsyncConsumer, which allows the callback
// to be async with lifetime ', which could allow the callback to borrow
// from the value passed to accept.
//
// This version of AsyncConsumer is not used in current codebase,
// but it could be useful in future when we want to have async callbacks
// that can borrow from the input value.

type BoxFuture = Pin<Box<dyn Future<Output = ()>>>;
pub(crate) struct AsyncConsumer<T> {
    ended_fn: Box<
        dyn for<'a> Fn(&'a T) -> BoxFuture<'a>
            + Send
            + Sync
    >,
}
impl<T: Send + Sync + 'static> AsyncConsumer<T> {
    pub(crate) fn new<F>(handler: F) -> Self
    where
        F: for<'a> Fn(&'a T) -> BoxFuture<'a>
            + Send
            + Sync
            + 'static,
    {
        Self {
            ended_fn: Box::new(handler),
        }
    }
    pub(crate) async fn accept(&self, value: &T) {
        (self.ended_fn)(value).await
    }
}
*/

#[cfg(test)]
mod unitests {
    use super::*;

    #[tokio::test]
    async fn test_consumer() {
        let consumer = Consumer::new(|value: &String| {
            assert!("hello".to_string() == *value);
        });
        consumer.accept(&"hello".to_string());
    }

    #[tokio::test]
    async fn test_async_consumer() {
        let consumer = AsyncConsumer::new(|value: String| {
            Box::pin(async move {
                assert!("hello".to_string() == value);
                /*
                use tokio::time::sleep;
                sleep(std::time::Duration::from_millis(100)).await;
                */
            })
        });
        consumer.accept("hello".to_string()).await;
    }
}
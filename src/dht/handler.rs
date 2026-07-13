pub(crate) struct Handler<T> {
    cb: Box<dyn Fn(&T)>,
}

impl<T: 'static> Handler<T> {
    pub(crate) fn new<F>(cb: F) -> Self
    where
        F: Fn(&T) + 'static
    {
        Self {
            cb: Box::new(cb),
        }
    }

    pub(crate) fn cb(&self, value: &T) {
        (self.cb)(value);
    }
}

type AsyncBoxFuture = futures::future::BoxFuture<'static, ()>;
pub(crate) struct AsyncHandler<T> {
    cb: Box<dyn Fn(T) -> AsyncBoxFuture + Send + 'static >
}

impl<T: 'static> AsyncHandler<T> {
    pub(crate) fn new<F>(handler: F) -> Self
    where
        F: Fn(T) -> AsyncBoxFuture + Send + 'static
    {
        Self {cb: Box::new(handler) }
    }

    pub(crate) async fn cb(&self, value: T) {
        (self.cb)(value).await;
    }
}

type LocalBoxFuture = futures::future::LocalBoxFuture<'static, ()>;
pub(crate) struct LocalHandler<T> {
    cb: Box<dyn Fn(T) -> LocalBoxFuture + 'static >
}

impl<T: 'static> LocalHandler<T> {
    pub(crate) fn new<F>(handler: F) -> Self
    where
        F: Fn(T) -> LocalBoxFuture + 'static
    {
        Self {cb: Box::new(handler) }
    }

    pub(crate) async fn cb(&self, value: T) {
        (self.cb)(value).await;
    }
}

pub(crate) trait Callable<T: 'static>: 'static {
    fn call_boxed(&self, value: T) -> futures::future::LocalBoxFuture<'static, ()>;
}

impl<T: 'static> Callable<T> for AsyncHandler<T> {
    fn call_boxed(&self, value: T) -> futures::future::LocalBoxFuture<'static, ()> {
        (self.cb)(value)
    }
}

impl<T: 'static> Callable<T> for LocalHandler<T> {
    fn call_boxed(&self, value: T) -> futures::future::LocalBoxFuture<'static, ()> {
        (self.cb)(value)
    }
}

#[cfg(test)]
mod unitests {
    use super::*;

    #[tokio::test]
    async fn test_consumer() {
        let hd = Handler::new(|value: &String| {
            assert!("hello".to_string() == *value);
        });
        hd.cb(&"hello".to_string());
    }

    #[tokio::test]
    async fn test_async_consumer() {
        let hd = AsyncHandler::new(|value: String| {
            Box::pin(async move {
                assert!("hello".to_string() == value);
            })
        });
        hd.cb("hello".to_string()).await;
    }
}
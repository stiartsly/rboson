
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

    #[allow(dead_code)]
    pub(crate) async fn cb(&self, value: T) {
        (self.cb)(value).await;
    }
}

pub(crate) trait Callable<T: 'static>: 'static {
    fn call_boxed(&self, value: T) -> futures::future::LocalBoxFuture<'static, ()>;
}

impl<T: 'static> Callable<T> for LocalHandler<T> {
    fn call_boxed(&self, value: T) -> futures::future::LocalBoxFuture<'static, ()> {
        (self.cb)(value)
    }
}

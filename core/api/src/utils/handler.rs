use futures::future;

pub struct EasyHandler<T> {
    cb: Box<dyn Fn(&T)>,
}

impl<T: 'static> EasyHandler<T> {
    pub fn new<F>(cb: F) -> Self
    where
        F: Fn(&T) + 'static,
    {
        Self { cb: Box::new(cb) }
    }

    pub fn cb(&self, value: &T) {
        (self.cb)(value);
    }
}

pub type BoxFuture = future::BoxFuture<'static, ()>;
pub struct BoxHandler<T> {
    cb: Box<dyn Fn(T) -> BoxFuture + Send + 'static>,
}

impl<T: 'static> BoxHandler<T> {
    pub fn new<F>(handler: F) -> Self
    where
        F: Fn(T) -> BoxFuture + Send + 'static,
    {
        Self {
            cb: Box::new(handler),
        }
    }

    #[allow(unused)]
    pub async fn cb(&self, value: T) {
        (self.cb)(value).await;
    }
}

type LocalBoxFuture = future::LocalBoxFuture<'static, ()>;
pub struct LocalBoxHandler<T> {
    cb: Box<dyn Fn(T) -> LocalBoxFuture + 'static>,
}

impl<T: 'static> LocalBoxHandler<T> {
    pub fn new<F>(handler: F) -> Self
    where
        F: Fn(T) -> LocalBoxFuture + 'static,
    {
        Self {
            cb: Box::new(handler),
        }
    }

    pub async fn cb(&self, value: T) {
        (self.cb)(value).await;
    }
}

pub trait Callable<T: 'static>: 'static {
    fn call_boxed(&self, value: T) -> future::LocalBoxFuture<'static, ()>;
}

impl<T: 'static> Callable<T> for BoxHandler<T> {
    fn call_boxed(&self, value: T) -> LocalBoxFuture {
        (self.cb)(value)
    }
}

impl<T: 'static> Callable<T> for LocalBoxHandler<T> {
    fn call_boxed(&self, value: T) -> LocalBoxFuture {
        (self.cb)(value)
    }
}

#[cfg(test)]
mod unitests {
    use super::*;

    #[tokio::test]
    async fn test_easy_consumer() {
        let hd = EasyHandler::new(|value: &String| {
            assert!("hello".to_string() == *value);
        });
        hd.cb(&"hello".to_string());
    }

    #[tokio::test]
    async fn test_box_consumer() {
        let hd = BoxHandler::new(|value: String| {
            Box::pin(async move {
                assert!("hello".to_string() == value);
            })
        });
        hd.cb("hello".to_string()).await;
    }

    #[tokio::test]
    async fn test_local_box_consumer() {
        let hd = LocalBoxHandler::new(|value: String| {
            Box::pin(async move {
                assert_eq!("hello", value);
            })
        });
        hd.cb("hello".to_string()).await;
    }
}

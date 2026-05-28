
pub(crate) struct Consumer<T> {
    ended_fn: Box<dyn Fn(T)>,
}

impl<T> Consumer<T> {
    pub(crate) fn new<F>(handler: F) -> Self
    where F: Fn(T) + 'static,{
        Self {
            ended_fn: Box::new(handler),
        }
    }

    pub(crate) fn accept(&self, value: T) {
        (self.ended_fn)(value);
    }
}

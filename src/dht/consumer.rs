
pub(crate) struct Consumer<> {
    ended_fn: Box<dyn Fn()>,
}

impl Consumer<> {
    pub(crate) fn new<F>(handler: F) -> Self
    where F: Fn() + 'static,{
        Self {
            ended_fn: Box::new(handler),
        }
    }

    pub(crate) fn accept(&self) {
        (self.ended_fn)();
    }
}

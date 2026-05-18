use super::task::Task;

pub(crate) trait TaskListener {
    fn started(&self, _task: &impl Task) {}
    fn completed(&self, _task: &impl Task) {}
    fn cancelled(&self, _task: &impl Task) {}
    fn ended(&self, _task: &impl Task);
}

pub(crate) struct DefListener {
    ended_fn: Box<dyn Fn(&dyn Task) + Send + Sync>,
}

impl DefListener {
    pub(crate) fn new<F>(ended_fn: F) -> Self
    where
        F: Fn(&dyn Task) + Send + Sync + 'static
    {
        Self {
            ended_fn: Box::new(ended_fn)
        }
    }
}

impl TaskListener for DefListener {
    fn ended(&self, _task: &impl Task) {
         (self.ended_fn)(_task);
    }
}

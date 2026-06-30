use super::task::Task;

#[derive(Default)]
pub(crate) struct TaskListener {
    started_fn:    Option<Box<dyn Fn(&dyn Task)>>,
    completed_fn:  Option<Box<dyn Fn(&dyn Task)>>,
    canceled_fn:   Option<Box<dyn Fn(&dyn Task)>>,
    ended_fn:      Option<Box<dyn Fn(&dyn Task)>>,
}

impl TaskListener {
    #[allow(unused)]
    pub(crate) fn started_fn<F>(mut self, f: F) -> Self
    where F: Fn(&dyn Task) + 'static {
        self.started_fn = Some(Box::new(f));
        self
    }

    #[allow(unused)]
    pub(crate) fn completed_fn<F>(mut self, f: F) -> Self
    where F: Fn(&dyn Task) + 'static {
        self.completed_fn  = Some(Box::new(f));
        self
    }

    #[allow(unused)]
    pub(crate) fn canceled_fn<F>(mut self, f: F) -> Self
    where F: Fn(&dyn Task) + 'static {
        self.canceled_fn = Some(Box::new(f));
        self
    }

    #[allow(unused)]
    pub(crate) fn ended_fn<F>(mut self, f: F) -> Self
    where F: Fn(&dyn Task) + 'static {
        self.ended_fn = Some(Box::new(f));
        self
    }

    pub(crate) fn started(&self, task: &dyn Task) {
        if let Some(f) = &self.started_fn {
            f(task);
        }
    }

    pub(crate) fn completed(&self, task: &dyn Task) {
        if let Some(f) = &self.completed_fn {
            f(task);
        }
    }

    pub(crate) fn canceled(&self, task: &dyn Task) {
        if let Some(f) = &self.canceled_fn {
            f(task);
        }
    }

    pub(crate) fn ended(&self, task: &dyn Task) {
        if let Some(f) = &self.ended_fn {
            f(task);
        }
    }
}
use super::task::Task;

pub(crate) struct TaskListener {
    started_fn:    Option<Box<dyn Fn(&dyn Task)>>,
    completed_fn:  Option<Box<dyn Fn(&dyn Task)>>,
    canceled_fn:   Option<Box<dyn Fn(&dyn Task)>>,
    ended_fn:      Option<Box<dyn Fn(&dyn Task)>>,
}

impl TaskListener {
    pub(crate) fn new() -> Self {
        Self {
            started_fn: None,
            completed_fn: None,
            canceled_fn: None,
            ended_fn: None,
        }
    }

    pub(crate) fn started_fn(mut self, f: Box<dyn Fn(&dyn Task)>) -> Self {
        self.started_fn = Some(f);
        self
    }

    pub(crate) fn completed_fn(mut self, f: Box<dyn Fn(&dyn Task)>) -> Self {
        self.completed_fn  = Some(f);
        self
    }

    pub(crate) fn canceled_fn(mut self, f: Box<dyn Fn(&dyn Task)>) -> Self {
        self.canceled_fn = Some(f);
        self
    }

    pub(crate) fn ended_fn(mut self, f: Box<dyn Fn(&dyn Task)>) -> Self {
        self.ended_fn = Some(f);
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
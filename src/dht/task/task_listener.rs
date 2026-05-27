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

    pub(crate) fn with_started_fn(&mut self, f: Box<dyn Fn(&dyn Task)>) -> &mut Self {
        self.started_fn = Some(f);
        self
    }

    pub(crate) fn with_completed_fn(&mut self, f: Box<dyn Fn(&dyn Task)>) -> &mut Self {
        self.completed_fn  = Some(f);
        self
    }

    pub(crate) fn with_canceled_fn(&mut self, f: Box<dyn Fn(&dyn Task)>) -> &mut Self {
        self.canceled_fn = Some(f);
        self
    }

    pub(crate) fn with_ended_fn(&mut self, f: Box<dyn Fn(&dyn Task)>) -> &mut Self {
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
// Copyright (c) 2023 -      bosonnetwork.io
//
// Permission is hereby granted, free of charge, to any person obtaining a copy
// of this software and associated documentation files (the "Software"), to deal
// in the Software without restriction, including without limitation the rights
// to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
// copies of the Software, and to permit persons to whom the Software is
// furnished to do so, subject to the following conditions:
//
// The above copyright notice and this permission notice shall be included in all
// copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
// IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
// FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
// AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
// LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
// OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
// SOFTWARE.

use super::task::Task;

#[derive(Default)]
pub(crate) struct TaskListener {
    started_fn: Option<Box<dyn Fn(&dyn Task)>>,
    completed_fn: Option<Box<dyn Fn(&dyn Task)>>,
    canceled_fn: Option<Box<dyn Fn(&dyn Task)>>,
    ended_fn: Option<Box<dyn Fn(&dyn Task)>>,
}

impl TaskListener {
    #[allow(unused)]
    pub(crate) fn started_fn<F>(mut self, f: F) -> Self
    where
        F: Fn(&dyn Task) + 'static,
    {
        self.started_fn = Some(Box::new(f));
        self
    }

    #[allow(unused)]
    pub(crate) fn completed_fn<F>(mut self, f: F) -> Self
    where
        F: Fn(&dyn Task) + 'static,
    {
        self.completed_fn = Some(Box::new(f));
        self
    }

    #[allow(unused)]
    pub(crate) fn canceled_fn<F>(mut self, f: F) -> Self
    where
        F: Fn(&dyn Task) + 'static,
    {
        self.canceled_fn = Some(Box::new(f));
        self
    }

    #[allow(unused)]
    pub(crate) fn ended_fn<F>(mut self, f: F) -> Self
    where
        F: Fn(&dyn Task) + 'static,
    {
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

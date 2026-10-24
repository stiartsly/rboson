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

use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc,
};
use tokio::sync::{mpsc, oneshot};

use super::handler::{BoxHandler, LocalBoxHandler};
use crate::errors::{Result, StateError};

pub type TimerId = u64;

pub enum TimerCmd<H> {
    Add {
        timer_id: TimerId,
        delay: u64,
        interval: Option<u64>,
        cb: H,
    },
    Cancel {
        timer_id: TimerId,
    },
    Stop {
        complete: oneshot::Sender<()>,
    },
}

#[derive(Clone)]
pub struct TimerClient<H> {
    sender: mpsc::UnboundedSender<TimerCmd<H>>,
    next_id: Arc<AtomicU64>,
}

impl<H> TimerClient<H> {
    pub fn new(sender: mpsc::UnboundedSender<TimerCmd<H>>) -> Self {
        Self {
            sender,
            next_id: Arc::new(AtomicU64::new(1)),
        }
    }

    fn next_timer_id(&self) -> TimerId {
        self.next_id.fetch_add(1, Ordering::Relaxed)
    }

    pub fn add_timer(
        &self,
        delay: u64,
        interval: Option<u64>,
        callback: H,
    ) -> Result<TimerId> {
        let timer_id = self.next_timer_id();
        self.sender
            .send(TimerCmd::Add {
                timer_id,
                delay,
                interval,
                cb: callback,
            })
            .map_err(|_| StateError::new("timer channel closed"))?;
        Ok(timer_id)
    }

    pub fn cancel_timer(&self, timer_id: TimerId) -> Result<()> {
        self.sender
            .send(TimerCmd::Cancel { timer_id })
            .map_err(|_| StateError::new("timer channel closed"))?;
        Ok(())
    }

    pub async fn stop_timers(&self) -> Result<()> {
        let (tx, rx) = oneshot::channel();
        self.sender
            .send(TimerCmd::Stop { complete: tx })
            .map_err(|_| StateError::new("timer channel closed"))?;

        rx.await
            .map_err(|_| StateError::new("timer shutdown acknowledgement dropped"))?;
        Ok(())
    }
}

// Aliases for standard (thread-safe Send) timer client
pub type BoxTimerCmd = TimerCmd<BoxHandler<()>>;
pub type BoxTimerClient = TimerClient<BoxHandler<()>>;

// Aliases for local (not Send) timer client
pub type LocalBoxTimerCmd = TimerCmd<LocalBoxHandler<()>>;
pub type LocalBoxTimerClient = TimerClient<LocalBoxHandler<()>>;

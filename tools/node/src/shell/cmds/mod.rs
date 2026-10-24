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

use std::sync::{Arc, Mutex};

macro_rules! println {
    () => {
        $crate::shell::cmds::print_result(String::new())
    };
    ($($arg:tt)*) => {
        $crate::shell::cmds::print_result(format!($($arg)*))
    };
}

macro_rules! green_println {
    () => {
        $crate::shell::cmds::print_result("\x1b[32m\x1b[0m".to_string())
    };
    ($($arg:tt)*) => {
        $crate::shell::cmds::print_result(format!("\x1b[32m{}\x1b[0m", format_args!($($arg)*)))
    };
}

macro_rules! red_print {
    () => {
        $crate::shell::cmds::print_result("\x1b[31m\x1b[0m".to_string())
    };
    ($($arg:tt)*) => {
        $crate::shell::cmds::print_result(format!("\x1b[31m{}\x1b[0m", format_args!($($arg)*)))
    };
}

type OutputHandler = Arc<dyn Fn(String) + Send + Sync>;

static RESULT_OUTPUT: Mutex<Option<OutputHandler>> = Mutex::new(None);


pub(crate) mod store_value;
pub(crate) mod announce_peer;
pub(crate) mod find_node;
pub(crate) mod find_peer;
pub(crate) mod find_value;
pub(crate) mod rt;
pub(crate) mod info;
pub(crate) mod log;


pub(crate) fn set_result_output(handler: impl Fn(String) + Send + Sync + 'static) {
    *RESULT_OUTPUT.lock().unwrap() = Some(Arc::new(handler));
}

pub(crate) fn print_result(line: String) {
    let handler = RESULT_OUTPUT.lock().unwrap().clone();
    if let Some(handler) = handler {
        handler(line);
    } else {
        std::println!("{line}");
    }
}

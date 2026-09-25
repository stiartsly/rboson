use std::sync::{Arc, Mutex};

macro_rules! println {
    () => {
        $crate::cmds::print_result(String::new())
    };
    ($($arg:tt)*) => {
        $crate::cmds::print_result(format!($($arg)*))
    };
}

type OutputHandler = Arc<dyn Fn(String) + Send + Sync>;

static RESULT_OUTPUT: Mutex<Option<OutputHandler>> = Mutex::new(None);

pub(crate) mod announce_peer;
pub(crate) mod find_node;
pub(crate) mod find_peer;
pub(crate) mod find_value;
pub(crate) mod info;
pub(crate) mod log;
pub(crate) mod routing_table;
pub(crate) mod store_value;

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

use boson::Id;
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
pub(crate) mod device;
pub(crate) mod find_node;
pub(crate) mod find_peer;
pub(crate) mod find_value;
pub(crate) mod identity;
pub(crate) mod log;
pub(crate) mod login;
pub(crate) mod me;
pub(crate) mod status;
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

pub(crate) fn parse_id(text: &str) -> Option<Id> {
    match Id::try_from(text) {
        Ok(id) => Some(id),
        Err(e) => {
            println!("\x1b[31mInvalid id '{text}': {e}\x1b[0m");
            None
        }
    }
}

use log::{LevelFilter, Metadata, Record};
use std::fs::{File, OpenOptions};
use std::io::{self, IoSlice, Write};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};

static mut MY_LOGGER: Option<Logger> = None;

type ConsoleOutputHandler = Arc<dyn Fn(String) + Send + Sync>;

struct Logger {
    console_output_enabled: AtomicBool,
    console_output_handler: Mutex<Option<ConsoleOutputHandler>>,
    max_level: LevelFilter,
    fp: Option<Arc<Mutex<File>>>,
}

impl log::Log for Logger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        metadata.level() <= self.max_level
    }

    fn log(&self, record: &Record) {
        if self.enabled(record.metadata()) {
            let record_target = record.target().rsplit("::").next().unwrap_or("N/A");
            let record_target = if record_target.len() > 8 {
                &record_target[0..8]
            } else {
                record_target
            };
            let record_level = format!("{}", record.level());
            let record_level = if record_level.len() > 4 {
                &record_level[0..4]
            } else {
                &record_level
            };
            let log = format!(
                "[{:<8}] [{:^4}] {}",
                record_target,
                record_level,
                record.args()
            );

            if let Some(fp) = self.fp.as_ref() {
                _ = fp
                    .lock()
                    .unwrap()
                    .write_vectored(&[IoSlice::new(log.as_bytes())]);
                _ = fp.lock().unwrap().write(b"\n");
            }

            if self.console_output_enabled.load(Ordering::Acquire) {
                let console_log = match record.level() {
                    log::Level::Error => format!("\x1b[31m{log}\x1b[0m"),
                    log::Level::Warn => format!("\x1b[33m{log}\x1b[0m"),
                    log::Level::Info => format!("\x1b[32m{log}\x1b[0m"),
                    _ => log,
                };
                let handler = self.console_output_handler.lock().unwrap().clone();
                if let Some(handler) = handler {
                    handler(console_log);
                } else {
                    println!("{console_log}");
                }
            }
        }
    }
    fn flush(&self) {
        io::stdout().flush().unwrap();
    }
}

impl Logger {
    pub(crate) fn new(max_level: LevelFilter, logfile: Option<&str>) -> Self {
        let mut logger = Self {
            console_output_enabled: AtomicBool::new(true),
            console_output_handler: Mutex::new(None),
            max_level,
            fp: None,
        };

        if let Some(file) = logfile {
            logger.fp = match OpenOptions::new().append(true).create(true).open(file) {
                Ok(fp) => Some(Arc::new(Mutex::new(fp))),
                Err(e) => {
                    println!("Failed to open log file {e}!!! Unable to log output to file.");
                    None
                }
            }
        }
        logger
    }
}

static NULL_LOGGER: NullLogger = NullLogger;
struct NullLogger;
impl log::Log for NullLogger {
    fn enabled(&self, _: &Metadata) -> bool {
        false
    }
    fn log(&self, _: &Record) {}
    fn flush(&self) {}
}

pub(crate) fn setup(max_level: LevelFilter, logfile: Option<&str>) {
    unsafe {
        MY_LOGGER = Some(Logger::new(max_level, logfile));
        if let Some(ref mut v) = MY_LOGGER {
            _ = log::set_logger(v);
            _ = log::set_max_level(v.max_level);
        }
    }
}

#[allow(unused)]
pub fn enable_console_output() {
    unsafe {
        if let Some(ref v) = MY_LOGGER {
            v.console_output_enabled.store(true, Ordering::Release);
        }
    }
}

#[allow(unused)]
pub fn disable_console_output() {
    unsafe {
        if let Some(ref v) = MY_LOGGER {
            v.console_output_enabled.store(false, Ordering::Release);
        }
    }
}

/// Sends console logs to `handler` rather than writing directly to stdout.
pub fn set_console_output_handler(handler: impl Fn(String) + Send + Sync + 'static) {
    unsafe {
        if let Some(ref v) = MY_LOGGER {
            *v.console_output_handler.lock().unwrap() = Some(Arc::new(handler));
        }
    }
}

pub(crate) fn teardown() {
    _ = log::set_logger(&NULL_LOGGER);
}

#[allow(unused)]
pub(crate) fn revert_console_output() {
    unsafe {
        if let Some(ref v) = MY_LOGGER {
            v.console_output_enabled.fetch_xor(true, Ordering::AcqRel);
        }
    }
}

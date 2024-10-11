use std::sync::{Arc, Mutex};
use std::io::{self, Write};
use std::fs::{File, OpenOptions};
use log::{
    Level,
    LevelFilter,
    Metadata,
    Record
};

use crate::{
    unwrap,
};

static mut MY_LOGGER: Option<Logger> = None;

struct  Logger {
    console_output_enabled: bool,
    max_level: LevelFilter,
    fp: Option<Arc<Mutex<File>>>,
}

impl log::Log for Logger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        metadata.level() <= Level::Info
    }

    fn log(&self, record: &Record) {
        if self.enabled(record.metadata()) {
            let log = format!("[{}] [{}] {}",
                record.target(),
                record.level(),
                record.args()
            );

            if let Some(fp) = self.fp.as_ref() {
                _ = fp.lock().unwrap().write(log.as_bytes());
            }

            if self.console_output_enabled {
                println!("{log}");
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
            console_output_enabled: true,
            max_level,
            fp: None,
        };

        if let Some(file) = logfile {
            logger.fp = match OpenOptions::new().append(true).open(file) {
                Ok(fp) => Some(Arc::new(Mutex::new(fp))),
                Err(e) => {
                    eprintln!("Failed to open log file {e}. Unable to log output to file.");
                    None
                }
            }
        }
        logger
    }

    pub(crate) fn revert_console_output(&mut self) {
        self.console_output_enabled = !self.console_output_enabled;
    }
}

static NULL_LOGGER: NullLogger = NullLogger;
struct NullLogger;
impl log::Log for NullLogger {
    fn enabled(&self, _: &Metadata) -> bool { false }
    fn log(&self, _: &Record) {}
    fn flush(&self) {}
}

pub(crate) fn setup(max_level: LevelFilter, logfile: Option<&str>) {
    unsafe {
        MY_LOGGER = Some(Logger::new(max_level, logfile));
        _ = log::set_logger(unwrap!(MY_LOGGER));
        _ = log::set_max_level(unwrap!(MY_LOGGER).max_level);
    }
}

pub(crate) fn teardown() {
    _ = log::set_logger(&NULL_LOGGER);
}

#[allow(dead_code)]
pub(crate) fn revert_console_output() {
    unsafe {
        MY_LOGGER.as_mut().map(|v| v.revert_console_output());
    }
}

pub(crate) fn convert_loglevel(loglevel: &str) -> LevelFilter {
    match loglevel {
        "trace" => LevelFilter::Trace,
        "debug" => LevelFilter::Debug,
        "info"  => LevelFilter::Info,
        "err" | "critical"   => LevelFilter::Error,
        "off" | _ => LevelFilter::Off
    }
}

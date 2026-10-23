use log::{Level, LevelFilter, Metadata, Record};
use std::{
    fs::{File, OpenOptions},
    io::{self, Write},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex, Once,
    },
};

const CRATE_NAME: &str = "boson";
const TARGET_WIDTH: usize = 8;
const LEVEL_WIDTH: usize = 4;

const COLOR_RESET: &str = "\x1b[0m";
const COLOR_ERR: &str   = "\x1b[31m";         // red
const COLOR_WARN: &str  = "\x1b[33m";      // yellow
const COLOR_INFO: &str  = "\x1b[32m";

static LOGGER: Logger = Logger::new();
static LOGGER_INIT: Once = Once::new();

type ConsoleOutputHandler = Arc<dyn Fn(String) + Send + Sync>;

struct LoggerState {
    max_level: LevelFilter,
    fp: Option<File>,
}

struct Logger {
    console_output_enabled: AtomicBool,
    console_output_handler: Mutex<Option<ConsoleOutputHandler>>,
    state: Mutex<LoggerState>,
}

impl Logger {
    const fn new() -> Self {
        Self {
            console_output_enabled: AtomicBool::new(true),
            console_output_handler: Mutex::new(None),
            state: Mutex::new(LoggerState {
                max_level: LevelFilter::Off,
                fp: None,
            }),
        }
    }

    fn configure(&self, max_level: LevelFilter, logfile: Option<&str>) {
        let fp = logfile.and_then(|file| {
            OpenOptions::new()
                .append(true)
                .create(true)
                .open(file)
                .map_err(|e| {
                    println!("Failed to open log file {file}: {e}. Unable to log output to file.")
                }).ok()
        });

        let mut state = self.state.lock().unwrap();
        state.max_level = max_level;
        state.fp = fp;
    }

    fn set_console_output(&self, enabled: bool) {
        self.console_output_enabled.store(enabled, Ordering::Release);
    }

    fn set_console_handler(&self, handler: Option<ConsoleOutputHandler>) {
        *self.console_output_handler.lock().unwrap() = handler;
    }

    /// Restores the console defaults: output enabled and no custom handler.
    fn reset_console(&self) {
        self.set_console_output(true);
        self.set_console_handler(None);
    }

    /// Builds the plain log line, or `None` when the record is not from this crate.
    fn format(record: &Record) -> Option<String> {
        let mut parts = record.target().split("::");
        if parts.next() != Some(CRATE_NAME) {
            return None;
        }

        let target = record.target().rsplit("::").next().unwrap_or("N/A");
        let target = abbreviate(target, TARGET_WIDTH);
        let level = abbreviate(record.level().as_str(), LEVEL_WIDTH);
        Some(format!(
            "[{target:<TARGET_WIDTH$}] [{level:^LEVEL_WIDTH$}] {}",
            record.args()
        ))
    }

    fn colorize(level: Level, line: String) -> String {
        match level {
            Level::Error => format!("{COLOR_ERR}{line}{COLOR_RESET}"),
            Level::Warn => format!("{COLOR_WARN}{line}{COLOR_RESET}"),
            Level::Info => format!("{COLOR_INFO}{line}{COLOR_RESET}"),
            _ => line,
        }
    }
}

impl log::Log for Logger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        metadata.level() <= self.state.lock().unwrap().max_level
    }

    fn log(&self, record: &Record) {
        if !self.enabled(record.metadata()) {
            return;
        }
        let Some(line) = Self::format(record) else {
            return;
        };

        if let Some(fp) = self.state.lock().unwrap().fp.as_mut() {
            _ = writeln!(fp, "{line}");
        }

        if !self.console_output_enabled.load(Ordering::Acquire) {
            return;
        }

        let line = Self::colorize(record.level(), line);
        let handler = self.console_output_handler.lock().unwrap().clone();
        match handler {
            Some(handler) => handler(line),
            _ => println!("{line}"),
        }
    }

    fn flush(&self) {
        _ = io::stdout().flush();
        if let Some(fp) = self.state.lock().unwrap().fp.as_mut() {
            _ = fp.flush();
        }
    }
}

fn abbreviate(value: &str, max_chars: usize) -> String {
    value.chars().take(max_chars).collect()
}

#[doc(hidden)]
pub fn setup(max_level: LevelFilter, logfile: Option<&str>) {
    LOGGER.configure(max_level, logfile);
    LOGGER.reset_console();

    LOGGER_INIT.call_once(|| {
        _ = log::set_logger(&LOGGER);
    });
    log::set_max_level(max_level);
}

#[doc(hidden)]
pub fn cleanup() {
    log::set_max_level(LevelFilter::Off);
    LOGGER.configure(LevelFilter::Off, None);
    LOGGER.reset_console();
}

// The following functions are used by the applications built on top of this crate.
pub fn enable_console_output() {
    LOGGER.set_console_output(true);
}

pub fn disable_console_output() {
    LOGGER.set_console_output(false);
}

pub fn set_console_output_handler(handler: impl Fn(String) + Send + Sync + 'static) {
    LOGGER.set_console_handler(Some(Arc::new(handler)));
}

#[cfg(test)]
#[path = "unitests/test_logger.rs"]
mod test_logger;

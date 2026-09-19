use log::{LevelFilter, Metadata, Record};
use std::fs::{File, OpenOptions};
use std::io::{self, Write};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex, Once,
};

static LOGGER: Logger = Logger::new();
static LOGGER_INIT: Once = Once::new();

type ConsoleOutputHandler = Arc<dyn Fn(String) + Send + Sync>;

struct Logger {
    console_output_enabled: AtomicBool,
    console_output_handler: Mutex<Option<ConsoleOutputHandler>>,
    state: Mutex<LoggerState>,
}

struct LoggerState {
    max_level: LevelFilter,
    fp: Option<File>,
}

impl log::Log for Logger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        metadata.level() <= self.state.lock().unwrap().max_level
    }

    fn log(&self, record: &Record) {
        if self.enabled(record.metadata()) {
            let record_target = abbreviate(record.target().rsplit("::").next().unwrap_or("N/A"), 8);
            let record_level = format!("{}", record.level());
            let record_level = abbreviate(&record_level, 4);
            let log = format!("[{record_target:<8}] [{record_level:^4}] {}", record.args());

            {
                let mut state = self.state.lock().unwrap();
                if let Some(fp) = state.fp.as_mut() {
                    _ = writeln!(fp, "{log}");
                }
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
        _ = io::stdout().flush();
        if let Some(fp) = self.state.lock().unwrap().fp.as_mut() {
            _ = fp.flush();
        }
    }
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
            match OpenOptions::new().append(true).create(true).open(file) {
                Ok(fp) => Some(fp),
                Err(e) => {
                    println!("Failed to open log file {file}: {e}. Unable to log output to file.");
                    None
                }
            }
        });

        let mut state = self.state.lock().unwrap();
        state.max_level = max_level;
        state.fp = fp;
    }
}

pub(crate) fn setup(max_level: LevelFilter, logfile: Option<&str>) {
    LOGGER.configure(max_level, logfile);
    LOGGER.console_output_enabled.store(true, Ordering::Release);
    *LOGGER.console_output_handler.lock().unwrap() = None;

    LOGGER_INIT.call_once(|| {
        _ = log::set_logger(&LOGGER);
    });
    log::set_max_level(max_level);
}

#[allow(unused)]
pub fn enable_console_output() {
    LOGGER.console_output_enabled.store(true, Ordering::Release);
}

#[allow(unused)]
pub fn disable_console_output() {
    LOGGER
        .console_output_enabled
        .store(false, Ordering::Release);
}

/// Sends console logs to `handler` rather than writing directly to stdout.
pub fn set_console_output_handler(handler: impl Fn(String) + Send + Sync + 'static) {
    *LOGGER.console_output_handler.lock().unwrap() = Some(Arc::new(handler));
}

pub(crate) fn teardown() {
    log::set_max_level(LevelFilter::Off);
    LOGGER.configure(LevelFilter::Off, None);
    *LOGGER.console_output_handler.lock().unwrap() = None;
    LOGGER.console_output_enabled.store(true, Ordering::Release);
}

#[allow(unused)]
pub(crate) fn revert_console_output() {
    LOGGER
        .console_output_enabled
        .fetch_xor(true, Ordering::AcqRel);
}

fn abbreviate(value: &str, max_chars: usize) -> String {
    value.chars().take(max_chars).collect()
}

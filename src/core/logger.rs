use std::io::{self, Write};
use log::{
    Level,
    LevelFilter,
    Metadata,
    Record
};

static MY_LOGGER: MyLogger = MyLogger;
struct MyLogger;
impl log::Log for MyLogger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        metadata.level() <= Level::Info
    }

    fn log(&self, record: &Record) {
        if self.enabled(record.metadata()) {
            println!(
                "[{}] [{}] {}",
                record.target(),
                record.level(),
                record.args()
            );
        }
    }
    fn flush(&self) {
        io::stdout().flush().unwrap();
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

pub(crate) fn setup() {
    _ = log::set_logger(&MY_LOGGER);
    _ = log::set_max_level(LevelFilter::Info);
}

pub(crate) fn teardown() {
    _ = log::set_logger(&NULL_LOGGER);
}

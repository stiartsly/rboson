use super::*;
use log::{debug, error, info, warn};
use std::{
    sync::Mutex as StdMutex,
    sync::Arc,
    path::PathBuf,
};

fn record_line(logger: &Logger, level: Level, target: &str, msg: &str) {
    use log::Log;
    logger.log(
        &Record::builder()
            .level(level)
            .target(target)
            .args(format_args!("{msg}"))
            .build(),
    );
}

fn capturing_logger(level: LevelFilter) -> (Logger, Arc<StdMutex<Vec<String>>>) {
    let logger = Logger::new();
    logger.configure(level, None);
    let lines = Arc::new(StdMutex::new(Vec::new()));
    let sink = lines.clone();
    logger.set_console_handler(Some(Arc::new(move |line| sink.lock().unwrap().push(line))));
    (logger, lines)
}

fn temp_log_path(name: &str) -> PathBuf {
    std::env::temp_dir().join(format!("boson_logger_{}_{name}.log", std::process::id()))
}

#[test]
fn test_logger() {
    setup(LevelFilter::Info, None);
    info!("info: testing....");
    error!("error: testing...");
    debug!("debug: testing...");
    warn!("warn: testing...");
    disable_console_output();
    info!("info: silenced");
    enable_console_output();
    set_console_output_handler(|_| {});
    info!("info: custom handler");
    cleanup();
}

#[test]
fn test_abbreviate() {
    assert_eq!(abbreviate("abcdefghij", 8), "abcdefgh");
    assert_eq!(abbreviate("abc", 8), "abc");
    assert_eq!(abbreviate("", 4), "");
    assert_eq!(abbreviate("héllo", 2), "hé");
}

#[test]
fn test_enabled_respects_max_level() {
    use log::Log;
    let logger = Logger::new();
    let meta = |level| Metadata::builder().level(level).build();

    assert!(!logger.enabled(&meta(Level::Error)));

    logger.configure(LevelFilter::Warn, None);
    assert!(logger.enabled(&meta(Level::Error)));
    assert!(logger.enabled(&meta(Level::Warn)));
    assert!(!logger.enabled(&meta(Level::Info)));
}

#[test]
fn test_format_line() {
    let record = Record::builder()
        .level(Level::Info)
        .target("boson::dht::node_runner_long")
        .args(format_args!("hello"))
        .build();
    assert_eq!(Logger::format(&record).unwrap(), "[node_run] [INFO] hello");

    let record = Record::builder()
        .level(Level::Warn)
        .target("boson::x")
        .args(format_args!("w"))
        .build();
    assert_eq!(Logger::format(&record).unwrap(), "[x       ] [WARN] w");
}

#[test]
fn test_colorize() {
    assert_eq!(
        Logger::colorize(Level::Error, "m".into()),
        format!("{COLOR_ERR}m{COLOR_RESET}")
    );
    assert_eq!(
        Logger::colorize(Level::Warn, "m".into()),
        format!("{COLOR_WARN}m{COLOR_RESET}")
    );
    assert_eq!(
        Logger::colorize(Level::Info, "m".into()),
        format!("{COLOR_INFO}m{COLOR_RESET}")
    );
    assert_eq!(Logger::colorize(Level::Debug, "m".into()), "m");
    assert_eq!(Logger::colorize(Level::Trace, "m".into()), "m");
}

#[test]
fn test_console_handler() {
    let (logger, lines) = capturing_logger(LevelFilter::Info);
    record_line(&logger, Level::Info, "boson::core::foo", "hi");
    record_line(&logger, Level::Error, "boson::core::foo", "bad");

    let lines = lines.lock().unwrap();
    assert_eq!(lines.len(), 2);
    assert_eq!(lines[0], format!("{COLOR_INFO}[foo     ] [INFO] hi{COLOR_RESET}"));
    assert!(lines[1].starts_with(COLOR_ERR));
}

#[test]
fn test_level_filtering() {
    let (logger, lines) = capturing_logger(LevelFilter::Warn);
    record_line(&logger, Level::Info, "boson::a", "dropped");
    record_line(&logger, Level::Warn, "boson::a", "kept");
    assert_eq!(lines.lock().unwrap().len(), 1);

    let (logger, lines) = capturing_logger(LevelFilter::Off);
    record_line(&logger, Level::Error, "boson::a", "dropped");
    assert!(lines.lock().unwrap().is_empty());
}

#[test]
fn test_console_output_toggle() {
    let (logger, lines) = capturing_logger(LevelFilter::Info);
    logger.set_console_output(false);
    record_line(&logger, Level::Info, "boson::a", "silent");
    assert!(lines.lock().unwrap().is_empty());

    logger.set_console_output(true);
    record_line(&logger, Level::Info, "boson::a", "audible");
    assert_eq!(lines.lock().unwrap().len(), 1);
}

#[test]
fn test_reset_console_output() {
    let (logger, lines) = capturing_logger(LevelFilter::Info);
    logger.set_console_output(false);
    logger.reset_console();

    assert!(logger.console_output_enabled.load(Ordering::Acquire));
    assert!(logger.console_output_handler.lock().unwrap().is_none());
    record_line(&logger, Level::Info, "boson::a", "to stdout");
    assert!(lines.lock().unwrap().is_empty());
}

#[test]
fn test_log_to_file() {
    use log::Log;
    let path = temp_log_path("file");
    _ = std::fs::remove_file(&path);

    let logger = Logger::new();
    logger.configure(LevelFilter::Info, path.to_str());
    logger.set_console_output(false);
    record_line(&logger, Level::Info, "boson::core::foo", "to file");
    record_line(&logger, Level::Debug, "boson::core::foo", "filtered");
    logger.flush();

    let content = std::fs::read_to_string(&path).unwrap();
    assert_eq!(content, "[foo     ] [INFO] to file\n");
    _ = std::fs::remove_file(&path);
}

#[test]
fn test_configure() {
    let path = temp_log_path("append");
    _ = std::fs::remove_file(&path);

    for msg in ["first", "second"] {
        let logger = Logger::new();
        logger.configure(LevelFilter::Info, path.to_str());
        logger.set_console_output(false);
        record_line(&logger, Level::Info, "boson::a", msg);
    }

    let content = std::fs::read_to_string(&path).unwrap();
    assert_eq!(content.lines().count(), 2);
    _ = std::fs::remove_file(&path);
}

#[test]
fn test_configure_ignores_invalid_file() {
    let logger = Logger::new();
    logger.configure(LevelFilter::Info, Some("/nonexistent_dir_boson/x.log"));
    let state = logger.state.lock().unwrap();
    assert!(state.fp.is_none());
    assert_eq!(state.max_level, LevelFilter::Info);
}

#[test]
fn test_configure_no_file_clears_previous() {
    let path = temp_log_path("drop");
    let logger = Logger::new();
    logger.configure(LevelFilter::Info, path.to_str());
    assert!(logger.state.lock().unwrap().fp.is_some());

    logger.configure(LevelFilter::Info, None);
    assert!(logger.state.lock().unwrap().fp.is_none());
    _ = std::fs::remove_file(&path);
}

use crate::core::logger;
use log::{debug, error, info};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_logger() {
        logger::setup(log::LevelFilter::Info, None);
        info!("info: testing....");
        error!("debug: testing...");
        assert!(true);
        logger::teardown();
    }

    #[test]
    fn test_logger_disable() {
        logger::setup(log::LevelFilter::Info, None);
        logger::revert_console_output();
        info!("info: testing....");
        debug!("debug: testing...");
        assert!(true);
        logger::teardown();
    }
}

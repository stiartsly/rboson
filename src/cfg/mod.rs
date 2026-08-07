pub mod configuration;
pub mod config;

#[cfg(test)]
mod unitests {
    mod test_configuration;
}

pub use {
    configuration::Configuration,
    config::{
        NodeConfig,
        ActiveProxyConfig,
    }
};


use std::any::Any;
use std::collections::HashMap;

pub trait ConfigAdapter {
    fn config(&self) -> HashMap<String, Box<dyn Any>>;
    fn on_config_updated(&self, config: HashMap<String, Box<dyn Any>>);
}

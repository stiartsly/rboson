use std::fmt;
use serde::Deserialize;

#[derive(Clone, Debug, Deserialize)]
pub enum Cycle {
    Monthly,
    Annually,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Plan {
    id: i32,
    name: String,

    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    detail: Option<String>,
    price: f64,
    currency: String,
    cycle: Cycle,
    #[serde(default)]
    annually_discount: f64,
    #[serde(default)]
    active: bool,
}

impl Plan {
    pub fn id(&self) -> i32 {
        self.id
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn description(&self) -> Option<&String> {
        self.description.as_ref()
    }

    pub fn detail(&self) -> Option<&String> {
        self.detail.as_ref()
    }

    pub fn price(&self) -> f64 {
        self.price
    }

    pub fn currency(&self) -> &str {
        &self.currency
    }

    pub fn cycle(&self) -> &Cycle {
        &self.cycle
    }

    pub fn annually_discount(&self) -> f64 {
        self.annually_discount
    }

    pub fn active(&self) -> bool {
        self.active
    }

    pub fn is_free(&self) -> bool {
        self.price == 0.0
    }
}

impl fmt::Display for Plan {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Plan {{ id: {}, name: {}, price: {}, currency: {}, cycle: {:?} }}",
            self.id, self.name, self.price, self.currency, self.cycle)
    }
}

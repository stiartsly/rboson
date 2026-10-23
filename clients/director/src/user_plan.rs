use super::{Plan, Subscription};
use std::fmt;

#[derive(Clone, Debug)]
pub struct UserPlan {
    pub name: String,
    pub plan: Option<Plan>,
    pub subscription: Option<Subscription>,
}

#[allow(dead_code)]
impl UserPlan {
    pub fn new(name: String, plan: Option<Plan>, subscription: Option<Subscription>) -> Self {
        Self {
            name,
            plan,
            subscription,
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn plan(&self) -> Option<&Plan> {
        self.plan.as_ref()
    }

    pub fn subscription(&self) -> Option<&Subscription> {
        self.subscription.as_ref()
    }
}

impl fmt::Display for UserPlan {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "UserPlan {{ name: {}, plan: {:?}, subscription: {:?} }}",
            self.name, self.plan, self.subscription
        )
    }
}

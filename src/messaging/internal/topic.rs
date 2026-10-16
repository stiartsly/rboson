pub(crate) enum Topic {
    UserInbox,
    UserOutbox,
    DeviceInbox,
    DeviceOutbox,
}

impl Topic {
    pub(crate) fn as_str(&self) -> &'static str {
        match self {
            Self::UserInbox => "u/i",
            Self::UserOutbox => "u/o",
            Self::DeviceInbox => "d/i",
            Self::DeviceOutbox => "d/o",
        }
    }

    pub(crate) fn into_inner(self) -> &'static str {
        self.as_str()
    }
}

impl From<&'static str> for Topic {
    fn from(value: &'static str) -> Self {
        match value {
            "u/i" => Topic::UserInbox,
            "u/o" => Topic::UserOutbox,
            "d/i" => Topic::DeviceInbox,
            "d/o" => Topic::DeviceOutbox,
            _ => panic!("Invalid topic name: {}", value),
        }
    }
}

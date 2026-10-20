#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum Topic {
    UserInbox,
    UserOutbox,
    DeviceInbox,
    DeviceOutbox,
}

impl Topic {
    pub(crate) const ALL: [Self; 4] = [
        Self::UserInbox,
        Self::UserOutbox,
        Self::DeviceInbox,
        Self::DeviceOutbox,
    ];

    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::UserInbox => "u/i",
            Self::UserOutbox => "u/o",
            Self::DeviceInbox => "d/i",
            Self::DeviceOutbox => "d/o",
        }
    }

    pub(crate) fn parse(value: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|topic| topic.as_str() == value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn topic_names_round_trip() {
        for topic in Topic::ALL {
            assert_eq!(Topic::parse(topic.as_str()), Some(topic));
        }
        assert_eq!(Topic::parse("unknown"), None);
    }
}

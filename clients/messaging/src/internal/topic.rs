// Copyright (c) 2023 -      bosonnetwork.io
//
// Permission is hereby granted, free of charge, to any person obtaining a copy
// of this software and associated documentation files (the "Software"), to deal
// in the Software without restriction, including without limitation the rights
// to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
// copies of the Software, and to permit persons to whom the Software is
// furnished to do so, subject to the following conditions:
//
// The above copyright notice and this permission notice shall be included in all
// copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
// IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
// FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
// AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
// LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
// OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
// SOFTWARE.

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

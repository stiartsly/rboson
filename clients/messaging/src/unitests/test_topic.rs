use crate::internal::topic::Topic;

#[test]
fn topic_names_round_trip() {
    for topic in Topic::ALL {
        assert_eq!(Topic::parse(topic.as_str()), Some(topic));
    }
    assert_eq!(Topic::parse("unknown"), None);
}

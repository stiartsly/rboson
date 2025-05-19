use std::time::SystemTime;
use crate::Id;
use crate::messaging::{
    contact::{ContactBuilder, ContactType},
};

#[test]
fn test_personal_contact() {
    let id = Id::random();
    let peerid = Id::random();
    let created = SystemTime::UNIX_EPOCH;
    let modified = SystemTime::UNIX_EPOCH;
    let revision = 1;

    let result = ContactBuilder::new(&id)
        .with_home_peerid(&peerid)
        .with_type(ContactType::Contact)
        .with_name("Alice")
        .with_avatar(true)
        .with_remark("Alice's remark")
        .with_tags("tags")
        .with_created(created)
        .with_last_modified(modified)
        .with_revision(revision)
        .build();

    assert_eq!(result.is_ok(), true);

    let mut contact = result.unwrap();
    assert_eq!(contact.id(), &id);
    assert_eq!(contact.home_peerid(), &peerid);

    assert_eq!(contact.name(), Some("Alice"));
    contact.set_name("Alice Smith");
    assert_eq!(contact.name(), Some("Alice Smith"));

    assert_eq!(contact.has_avatar(), true);
    assert_eq!(contact.avatar_url().is_some(), true);
    assert_eq!(contact.avatar_url().unwrap(), format!("bmr://{}/{}", contact.home_peerid(), contact.id()));

    contact.set_avatar(false);
    assert_eq!(contact.has_avatar(), false);
    assert_eq!(contact.avatar_url().is_some(), false);

    assert_eq!(contact.remark().is_some(), true);
    assert_eq!(contact.remark().unwrap(), "Alice's remark");
    contact.set_remark("Alice's new remark");
    assert_eq!(contact.remark().is_some(), true);
    assert_eq!(contact.remark().unwrap(), "Alice's new remark");
    contact.set_remark("");
    assert_eq!(contact.remark().is_some(), false);

    assert_eq!(contact.tags().is_some(), true);
    assert_eq!(contact.tags().unwrap(), "tags");
    contact.set_tags("new tags");
    assert_eq!(contact.tags().is_some(), true);
    assert_eq!(contact.tags().unwrap(), "new tags");
    contact.set_tags("");
    assert_eq!(contact.tags().is_some(), false);

    assert_eq!(contact.is_muted(), false);
    contact.set_muted(true);
    assert_eq!(contact.is_muted(), true);

    assert_eq!(contact.is_blocked(), false);
    contact.set_blocked(true);
    assert_eq!(contact.is_blocked(), true);

    assert_eq!(contact.is_delted(), false);
    contact.set_deleted(true);
    assert_eq!(contact.is_delted(), true);

    assert_eq!(contact.created(), created);
    assert_eq!(contact.last_modified() > modified, true);
    assert_eq!(contact.is_modified(), true);
    assert_eq!(contact.revision() > revision, true);

}

#[ignore]
#[test]
fn test_group_contact() {
    unimplemented!();
}
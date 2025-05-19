use crate::Id;

use crate::messaging::{
    channel::Permission,
    channel::Role,
    channel::Member,
};

#[test]
fn test_permission_try_from() {
    let permission = Permission::try_from(0).unwrap();
    assert_eq!(permission, Permission::Public);
    assert_eq!(permission as i32, 0);
    assert_eq!(permission.to_string(), "Public");

    let serialized = serde_json::to_string(&permission).unwrap();
    println!("Serialized permission: {}", serialized);
    let deserialized: Permission = serde_json::from_str(&serialized).unwrap();
    assert_eq!(permission, deserialized);

    let permission = Permission::try_from(1).unwrap();
    assert_eq!(permission, Permission::MemberInvite);
    assert_eq!(permission as i32, 1);
    assert_eq!(permission.to_string(), "MemberInvite");

    let permission = Permission::try_from(2).unwrap();
    assert_eq!(permission, Permission::ModeratorInvite);
    assert_eq!(permission as i32, 2);
    assert_eq!(permission.to_string(), "ModeratorInvite");

    let permission = Permission::try_from(3).unwrap();
    assert_eq!(permission, Permission::OwnerInvite);
    assert_eq!(permission as i32, 3);
    assert_eq!(permission.to_string(), "OwnerInvite");

    let permission = Permission::try_from(-1);
    assert!(permission.is_err());
    assert_eq!(permission.unwrap_err(), "Invalid permission value");

    let permission = Permission::try_from(4);
    assert!(permission.is_err());
    assert_eq!(permission.unwrap_err(), "Invalid permission value");
}

#[test]
fn test_role_try_from() {
    let role = Role::try_from(0).unwrap();
    assert_eq!(role, Role::Owner);
    assert_eq!(role as i32, 0);
    assert_eq!(role.to_string(), "Owner");

    let role = Role::try_from(1).unwrap();
    assert_eq!(role, Role::Moderator);
    assert_eq!(role as i32, 1);
    assert_eq!(role.to_string(), "Moderator");

    let role = Role::try_from(2).unwrap();
    assert_eq!(role, Role::Member);
    assert_eq!(role as i32, 2);
    assert_eq!(role.to_string(), "Member");

    let role = Role::try_from(-1).unwrap();
    assert_eq!(role, Role::Banned);
    assert_eq!(role as i32, -1);
    assert_eq!(role.to_string(), "Banned");

    let role = Role::try_from(3);
    assert!(role.is_err());
    assert_eq!(role.unwrap_err(), "Invalid role value");

    let role = Role::try_from(-2);
    assert!(role.is_err());
    assert_eq!(role.unwrap_err(), "Invalid role value");
}

#[test]
fn test_member() {
    let userid = Id::random();
    let peerid = Id::random();
    let member = Member::new(&userid, &peerid, Role::Owner, 1234567890);

    assert_eq!(member.id(), &userid);
    assert_eq!(member.role(), Role::Owner);
    assert_eq!(member.is_owner(), true);
    assert_eq!(member.is_moderator(), false);
    assert_eq!(member.is_banned(), false);
    assert_eq!(member.joined(), 1234567890);

    let serialized = serde_json::to_string(&member).unwrap();
    println!("Serialized member: {}", serialized);
    let deserialized: Member = serde_json::from_str(&serialized).unwrap();
    assert_eq!(member, deserialized);

    let member = Member::new(&userid, &peerid, Role::Moderator, 1234567890);

    assert_eq!(member.id(), &userid);
    assert_eq!(member.role(), Role::Moderator);
    assert_eq!(member.is_owner(), false);
    assert_eq!(member.is_moderator(), true);
    assert_eq!(member.is_banned(), false);
    assert_eq!(member.joined(), 1234567890);

    let member = Member::new(&userid, &peerid, Role::Banned, 1234567890);

    assert_eq!(member.id(), &userid);
    assert_eq!(member.role(), Role::Banned);
    assert_eq!(member.is_owner(), false);
    assert_eq!(member.is_moderator(), false);
    assert_eq!(member.is_banned(), true);
    assert_eq!(member.joined(), 1234567890);
}

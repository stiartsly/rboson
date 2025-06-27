use boson::Id;
use boson::did::{
    DID_SCHEME,
    DID_METHOD,
    DIDUrl
};

/*  APIs for testcase
 - DIDUrl::parse(..)        [X]
 - DIDUrl::from_id(..)      [X]
 - DIDUrl::from(..)         [X]
 - Eq
 - PartialEq
 */

 #[test]
 fn test_parse() {
    let id = Id::random();
    let rc = DIDUrl::parse(&id.to_did_string());
    assert!(rc.is_ok());

    let url1 = rc.unwrap();
    assert_eq!(url1.scheme(), DID_SCHEME);
    assert_eq!(url1.method(), DID_METHOD);
    assert_eq!(url1.id(), &id);
    assert_eq!(url1.path(), None);
    assert_eq!(url1.query(), None);
    assert_eq!(url1.fragment(), None);

    let url2 = DIDUrl::from_id(&id);
    assert_eq!(url1.scheme(), DID_SCHEME);
    assert_eq!(url1.method(), DID_METHOD);
    assert_eq!(url1.id(), &id);
    assert_eq!(url1.path(), None);
    assert_eq!(url1.query(), None);
    assert_eq!(url1.fragment(), None);
    assert_eq!(url1, url2);
}

use boson::Id;
use boson::did::{
    DID_SCHEME,
    DID_METHOD,
    DIDUrl
};

/*  APIs for testcase
 - DIDUrl::parse(..)        [X]
 - DIDUrl::from_id(..)      [X]
 - DIDUrl::from(bytes)      [X]
 - DIDUrl::try_from(str)    [X]
 - Eq
 - PartialEq
 */

 #[test]
 fn test_parse() {
    let id = Id::random();
    let rc = DIDUrl::parse(&id.to_did_string());
    assert!(rc.is_ok());

    let url1 = rc.unwrap();
    println!("Parsed URL: {}", url1);

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

#[test]
fn test_from_id() {
    let id = Id::random();
    let url = DIDUrl::from(&id);
    assert_eq!(url.scheme(), DID_SCHEME);
    assert_eq!(url.method(), DID_METHOD);
    assert_eq!(url.id(), &id);
    assert_eq!(url.path(), None);
    assert_eq!(url.query(), None);
    assert_eq!(url.fragment(), None);
}

#[test]
fn test_from_str() {
    let id = Id::random();
    let rc = DIDUrl::try_from(id.to_did_string().as_str());
    assert!(rc.is_ok());

    let url = rc.unwrap();
    assert_eq!(url.scheme(), DID_SCHEME);
    assert_eq!(url.method(), DID_METHOD);
    assert_eq!(url.id(), &id);
    assert_eq!(url.path(), None);
    assert_eq!(url.query(), None);
    assert_eq!(url.fragment(), None);
}

#[test]
#[ignore]
fn test_full_didurl() {
    /*let id = Id::random();
    let path = "path/to/resource";
    let query = "query=param";
    let fragment = "fragment";
    let did_url_str = format!("{}?query#fragment", id.to_did_string, "", id.to_did_string());
    let did_url_str = format!("{}:{}#{}", DID_SCHEME, id.to_did_string(), "fragment");

    let rc = DIDUrl::parse(&did_url_str);
    assert!(rc.is_ok());

    let url = rc.unwrap();
    assert_eq!(url.scheme(), DID_SCHEME);
    assert_eq!(url.method(), DID_METHOD);
    assert_eq!(url.id(), &id);
    assert_eq!(url.path(), None);
    assert_eq!(url.query(), None);
    assert_eq!(url.fragment(), Some("fragment"));*/
}

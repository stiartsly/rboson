use crate::Id;
use crate::did::{
    DIDUrl,
    DID_SCHEME,
    DID_METHOD,
};

#[test]
fn test_didurl() {
    let id = Id::random();
    let url = DIDUrl::new(&id, None, None, None);
    assert_eq!(url.scheme(), DID_SCHEME);
    assert_eq!(url.method(), DID_METHOD);
    assert_eq!(url.id(), &id);
    assert_eq!(url.path(), None);
    assert_eq!(url.query(), None);
    assert_eq!(url.fragment(), None);

    let url2 = DIDUrl::from_id(&id);
    assert_eq!(url, url2);
}

#[test]
fn test_didurl_full() {
    let id = Id::random();
    let path = "path/to/resource";
    let query = "query=param";
    let fragment = "fragment";
    let url = DIDUrl::new(&id, Some(path), Some(query), Some(fragment));
    assert_eq!(url.scheme(), DID_SCHEME);
    assert_eq!(url.method(), DID_METHOD);
    assert_eq!(url.id(), &id);
    assert_eq!(url.path(), Some(path));
    assert_eq!(url.query(), Some(query));
    assert_eq!(url.fragment(), Some(fragment));

    println!("DIDUrl: {}", url);

    let rc = DIDUrl::parse(&url.to_string());
    println!("Parsed DIDUrl: {:?}", rc);
    //assert!(rc.is_ok());

    //let url2 = rc.unwrap();
    //assert_eq!(url, url2);
}

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

use boson::{
    did::{DIDUrl, DID_METHOD, DID_SCHEME},
    Id,
};

/*  APIs for testcase
- DIDUrl::parse(..)        [X]
- DIDUrl::from_id(..)      [X]
- DIDUrl::from(bytes)      [X]
- DIDUrl::try_from(str)    [X]
- Eq
- PartialEq
*/

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse() {
        let id = Id::random();
        let rc = DIDUrl::parse(&id.to_did_string());
        assert!(rc.is_ok());

        let url1 = rc.unwrap();
        println!("Parsed URL: {}", url1);

        assert_eq!(url1.scheme(), Some(DID_SCHEME));
        assert_eq!(url1.method(), Some(DID_METHOD));
        assert_eq!(url1.id(), Some(&id));
        assert_eq!(url1.path(), None);
        assert_eq!(url1.query(), None);
        assert_eq!(url1.fragment(), None);

        let url2 = DIDUrl::from_id(&id);
        assert_eq!(url1.scheme(), Some(DID_SCHEME));
        assert_eq!(url1.method(), Some(DID_METHOD));
        assert_eq!(url1.id(), Some(&id));
        assert_eq!(url1.path(), None);
        assert_eq!(url1.query(), None);
        assert_eq!(url1.fragment(), None);
        assert_eq!(url1, url2);
    }

    #[test]
    fn test_from_id() {
        let id = Id::random();
        let url = DIDUrl::from(&id);
        assert_eq!(url.scheme(), Some(DID_SCHEME));
        assert_eq!(url.method(), Some(DID_METHOD));
        assert_eq!(url.id(), Some(&id));
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
        assert_eq!(url.scheme(), Some(DID_SCHEME));
        assert_eq!(url.method(), Some(DID_METHOD));
        assert_eq!(url.id(), Some(&id));
        assert_eq!(url.path(), None);
        assert_eq!(url.query(), None);
        assert_eq!(url.fragment(), None);
    }

    #[test]
    fn test_full_didurl() {
        let id = Id::random();
        let didurl = format!("did:boson:{}/path?query#key-1", id.to_string());
        let rc = DIDUrl::parse(&didurl);
        assert!(rc.is_ok());

        let url = rc.unwrap();
        assert_eq!(url.scheme(), Some(DID_SCHEME));
        assert_eq!(url.method(), Some(DID_METHOD));
        assert_eq!(url.id(), Some(&id));
        assert_eq!(url.path(), Some("/path"));
        assert_eq!(url.query(), Some("query"));
        assert_eq!(url.fragment(), Some("key-1"));

        let rc = DIDUrl::try_from(didurl.as_str());
        assert!(rc.is_ok());
        let url2 = rc.unwrap();
        assert_eq!(url, url2);
    }

    #[test]
    fn test_didurl_with_path_only() {
        let id = Id::random();
        let didurl = format!("did:boson:{}/path", id.to_string());
        let rc = DIDUrl::parse(&didurl);
        assert!(rc.is_ok());

        let url = rc.unwrap();
        assert_eq!(url.scheme(), Some(DID_SCHEME));
        assert_eq!(url.method(), Some(DID_METHOD));
        assert_eq!(url.id(), Some(&id));
        assert_eq!(url.path(), Some("/path"));
        assert_eq!(url.query(), None);
        assert_eq!(url.fragment(), None);

        let rc = DIDUrl::try_from(didurl.as_str());
        assert!(rc.is_ok());
        let url2 = rc.unwrap();
        assert_eq!(url, url2);
    }

    #[test]
    fn test_didurl_with_query_only() {
        let id = Id::random();
        let didurl = format!("did:boson:{}?query", id.to_string());
        let rc = DIDUrl::parse(&didurl);
        assert!(rc.is_ok());

        let url = rc.unwrap();
        assert_eq!(url.scheme(), Some(DID_SCHEME));
        assert_eq!(url.method(), Some(DID_METHOD));
        assert_eq!(url.id(), Some(&id));
        assert_eq!(url.path(), None);
        assert_eq!(url.query(), Some("query"));
        assert_eq!(url.fragment(), None);

        let rc = DIDUrl::try_from(didurl.as_str());
        assert!(rc.is_ok());
        let url2 = rc.unwrap();
        assert_eq!(url, url2);
    }

    #[test]
    fn test_didurl_with_fragment_only() {
        let id = Id::random();
        let didurl = format!("did:boson:{}#key-1", id.to_string());
        let rc = DIDUrl::parse(&didurl);
        assert!(rc.is_ok());

        let url = rc.unwrap();
        assert_eq!(url.scheme(), Some(DID_SCHEME));
        assert_eq!(url.method(), Some(DID_METHOD));
        assert_eq!(url.id(), Some(&id));
        assert_eq!(url.path(), None);
        assert_eq!(url.query(), None);
        assert_eq!(url.fragment(), Some("key-1"));

        let rc = DIDUrl::try_from(didurl.as_str());
        assert!(rc.is_ok());
        let url2 = rc.unwrap();
        assert_eq!(url, url2);
    }
}

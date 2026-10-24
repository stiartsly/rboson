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

use crate::{
    {DIDUrl, DID_METHOD, DID_SCHEME},
};
use boson_api::{
    Id,
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_didurl() {
        let id = Id::random();
        let url = DIDUrl::new(&id, None, None, None);
        assert_eq!(url.scheme(), Some(DID_SCHEME));
        assert_eq!(url.method(), Some(DID_METHOD));
        assert_eq!(url.id(), Some(&id));
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
        assert_eq!(url.scheme(), Some(DID_SCHEME));
        assert_eq!(url.method(), Some(DID_METHOD));
        assert_eq!(url.id(), Some(&id));
        assert_eq!(url.path(), Some(path));
        assert_eq!(url.query(), Some(query));
        assert_eq!(url.fragment(), Some(fragment));

        println!("DIDUrl: {}", url);

        let rc = DIDUrl::parse(&url.to_string());
        println!("Parsed DIDUrl: {:?}", rc);
        assert!(rc.is_ok());

        let url2 = rc.unwrap();
        assert_eq!(url, url2);
    }
}

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

use super::ContentDisposition;

#[test]
fn parses_filename() {
    let disposition = ContentDisposition::parse(
        "attachment; filename=\"file_name.jpg\"; filename*=UTF-8''file%20name.jpg",
    )
    .unwrap();

    assert!(disposition.is_attachment());
    assert_eq!(disposition.filename(), Some("file name.jpg"));
    assert_eq!(disposition.ascii_filename(), Some("file_name.jpg"));
    assert_eq!(
        disposition.rfc5987_filename(),
        Some("UTF-8''file%20name.jpg")
    );
}

#[test]
fn creates_attachment() {
    let disposition = ContentDisposition::attachment("fïle nãme.jpg");

    assert_eq!(disposition.filename(), Some("fïle nãme.jpg"));
    assert_eq!(disposition.ascii_filename(), Some("file name.jpg"));
    assert_eq!(
        disposition.value(),
        "attachment; filename=\"file name.jpg\"; filename*=UTF-8''f%C3%AFle%20n%C3%A3me.jpg"
    );
}

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

#[cfg(test)]
mod tests {
    use boson::{
        Id,
        messaging::{Client, ContentDisposition, Options},
        signature::KeyPair,
    };

    fn client() -> Client {
        let mut builder = Options::builder();
        builder.with_peer_id(Id::from(KeyPair::random().public_key()));
        builder.with_peer_endpoint("mqtt://127.0.0.1:1883").unwrap();
        builder.with_user_keypair(KeyPair::random());
        builder.with_device_keypair(KeyPair::random());
        Client::new(builder.build().unwrap())
    }

    #[tokio::test]
    async fn test_message_builder_public_construction() {
        let client = client();
        let missing_recipient = client
            .message(None)
            .text_body("message")
            .send()
            .await
            .err()
            .unwrap()
            .to_string();
        assert!(missing_recipient.contains("Message recipient is required"));

        let missing_body = client
            .message(Some(Id::from(KeyPair::random().public_key())))
            .send()
            .await
            .err()
            .unwrap()
            .to_string();
        assert!(missing_body.contains("Message content is required"));

        let recipient = Id::from(KeyPair::random().public_key());
        let not_running = client
            .message(Some(recipient))
            .content_type("text/plain")
            .content_disposition(ContentDisposition::attachment("message.txt"))
            .header("X-Message-Test", "builder")
            .text_body("builder message")
            .send()
            .await
            .err()
            .unwrap()
            .to_string();
        assert!(not_running.contains("Messaging client is not running"));

        let binary_not_running = client
            .message(Some(recipient))
            .content_type("application/octet-stream")
            .binary_body(vec![0, 1, 2])
            .send()
            .await
            .err()
            .unwrap()
            .to_string();
        assert!(binary_not_running.contains("Messaging client is not running"));
    }
}

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

use boson::activeproxy::{Client, Options};
use boson::{signature::KeyPair, Id};

#[cfg(test)]
mod tests {
    use super::*;

    fn options(service_peerid: &Id, service: &str) -> Options {
        let user_key = KeyPair::random();
        let device_key = KeyPair::random();
        let user_id = Id::from(user_key.public_key());
        Options::read(format!(
            "service:\n  peerId: {service_peerid}\n{service}\
             client:\n  userId: {user_id}\n  userPrivateKey: \"{}\"\n  devicePrivateKey: \"{}\"\n\
             upstream:\n  host: 127.0.0.1\n  port: 8080\n  scheme: http://\n",
            user_key.private_key(),
            device_key.private_key(),
        ))
        .unwrap()
    }

    #[test]
    fn configured_service_host_is_used() {
        let service_peerid = Id::random();
        let options = options(&service_peerid, "  host: 192.0.2.1\n  port: 9091\n");

        let client = Client::new(options).unwrap();

        assert_eq!(client.service_endpoint().as_deref(), Some("192.0.2.1:9091"));
    }

    #[test]
    fn service_host_is_used_without_node_or_peer() {
        let service_peerid = Id::random();
        let options = options(&service_peerid, "  host: 192.0.2.1\n  port: 9091\n");

        let client = Client::new(options).unwrap();

        assert_eq!(client.service_endpoint().as_deref(), Some("192.0.2.1:9091"));
    }
}

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

use std::{
    fs,
    time::{SystemTime, UNIX_EPOCH},
};

use crate::{
    {Node, NodeOptions},
};
use boson_api::{
    signature::KeyPair,
};

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test(flavor = "current_thread")]
    async fn test_concurrent_start_not_allowed() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let data_dir = std::env::temp_dir().join(format!(
            "boson-node-lifecycle-{}-{unique}",
            std::process::id(),
        ));
        let sk = KeyPair::random().to_private_key();
        let options = NodeOptions::new(sk)
            .with_host4("127.0.0.1")
            .with_port(39111)
            .with_data_dir(data_dir.to_str().unwrap());
        let node = Node::new(options).unwrap();

        let (first, second) = tokio::join!(node.start(), node.start());
        assert_ne!(first.is_ok(), second.is_ok());
        assert!(node.is_running());

        node.stop().await.unwrap();
        fs::remove_dir_all(data_dir).unwrap();
    }

    #[test]
    fn node_info_requires_running_node() {
        let data_dir =
            std::env::temp_dir().join(format!("boson-node-info-{}", std::process::id(),));
        let sk = KeyPair::random().to_private_key();
        let options = NodeOptions::new(sk)
            .with_host4("127.0.0.1")
            .with_port(39112)
            .with_data_dir(data_dir.to_str().unwrap());
        let node = Node::new(options).unwrap();

        assert!(node.node_info().is_err());

        fs::remove_dir_all(data_dir).unwrap();
    }
}

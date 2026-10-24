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

use std::{net::SocketAddr, thread, time::Duration};

use crate::{token_manager::TokenManager};
use boson_api::{Id};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_token() {
        let man = TokenManager::new();

        let nodeid = Id::random();
        let target = Id::random();
        let addr = "192.168.1.123:32222".parse::<SocketAddr>().unwrap();
        thread::sleep(Duration::from_secs(1));

        let token1 = man.generate_token(&nodeid, &addr, &target);
        let token2 = man.generate_token(&nodeid, &addr, &target);
        assert_eq!(token1, token2);
    }

    #[test]
    fn test_verify_token() {
        let man = TokenManager::new();

        let nodeid = Id::random();
        let target = Id::random();
        let addr = "192.168.1.123:32222".parse::<SocketAddr>().unwrap();
        thread::sleep(Duration::from_secs(1));

        let token = man.generate_token(&nodeid, &addr, &target);
        let result = man.verify_token(token, &nodeid, &addr, &target);
        assert_eq!(result, true);
    }
}

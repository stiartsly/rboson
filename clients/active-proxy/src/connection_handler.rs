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

use std::{rc::Rc, net::SocketAddr};
use futures::future::LocalBoxFuture;
use tokio::net::TcpStream;

use super::connection::ProxyConnection;
use boson_api::{cryptobox, CryptoContext, Result};

pub(crate) trait ConnectionHandler {
    fn challenge(&self, connection: &Rc<ProxyConnection>, challenge: &[u8]);

    fn authenticated(
        &self,
        connection: &Rc<ProxyConnection>,
        server_session_pk: &cryptobox::PublicKey,
        max_connections: i32,
        name_access: bool,
        endpoint: &str,
        named_endpoint: Option<&str>,
    ) -> Option<CryptoContext>;

    fn open(&self, connection: &Rc<ProxyConnection>);

    fn close(&self, connection: &Rc<ProxyConnection>);

    fn idle(&self, connection: &Rc<ProxyConnection>);

    fn busy(&self, connection: &Rc<ProxyConnection>);

    fn allow(&self, client_addr: SocketAddr) -> bool;

    fn connect_upstream(&self) -> LocalBoxFuture<'_, Result<TcpStream>>;
}

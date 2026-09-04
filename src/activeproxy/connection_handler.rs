use std::{net::SocketAddr, rc::Rc};
use futures::future::LocalBoxFuture;
use tokio::net::TcpStream;

use crate::{
    cryptobox,
    CryptoContext,
    Result,
};
use super::connection::ProxyConnection;

pub(crate) trait ConnectionHandler {
    fn challenge(
        &self,
        connection: &Rc<ProxyConnection>,
        challenge: &[u8],
    );

    fn authenticated(
        &self,
        connection: &Rc<ProxyConnection>,
        server_session_pk: &cryptobox::PublicKey,
        max_connections: i32,
        name_access: bool,
        endpoint: &str,
        named_endpoint: Option<&str>,
    ) -> Option<CryptoContext>;

    fn open(
        &self,
        connection: &Rc<ProxyConnection>
    );

    fn close(
        &self,
        connection: &Rc<ProxyConnection>
    );

    fn idle(
        &self,
        connection: &Rc<ProxyConnection>
    );

    fn busy(
        &self,
        connection: &Rc<ProxyConnection>
    );

    fn allow(&self, client_addr: SocketAddr) -> bool;

    fn connect_upstream(&self) -> LocalBoxFuture<'_, Result<TcpStream>>;
}

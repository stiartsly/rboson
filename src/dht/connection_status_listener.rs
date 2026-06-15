use crate::Network;
use crate::dht::ConnectionStatus;

/// A listener for connection status changes in the DHT network.
///
pub trait ConnectionStatusListener: Send + Sync {
    ///
    /// Called when the connection status of the Boson node changes.
    ///
    /// @param network the DHT network, IPv4 or IPv6.
    /// @param newStatus the new connection status.
    /// @param oldStatus the old connection status.
    ///
    fn status_changed(&self,
        _network: Network,
        _new_status: ConnectionStatus,
        _old_status: ConnectionStatus,
    ) {}

    ///
    /// Called when the Boson node is connecting to the Boson network.
    ///
    /// @param network the DHT network, IPv4 or IPv6.
    ///
    fn connecting(&self, _network: Network) {}

    ///
    /// Called when the Boson node has established a connection to the Boson network.
    ///
    /// @param network the DHT network, IPv4 or IPv6.
    ///
    fn connected(&self, _network: Network) {}

    /// Called when the Boson node has lost connection to the Boson network.
    ///
    /// @param network the DHT network, IPv4 or IPv6.
    ///
    fn disconnected(&self, _network: Network) {}
}

use crate::Network;
use crate::dht::ConnectionStatus;

pub trait ConnectionStatusListener {
    fn status_changed(&self,
        _network: Network,
        _new_status: ConnectionStatus,
        _old_status: ConnectionStatus,
    ) {}

    fn connected(&self, _network: Network) {}
    fn profound(&self, _network: Network) {}
    fn disconnected(&self, _network: Network) {}
}

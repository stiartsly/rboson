use std::sync::{Arc, Mutex};
use std::time::Duration;
use std::thread;
use clap::Parser;

use boson::{
    configuration as cfg,
    signature,
    Id,
    dht::Node,
    appdata_store::AppDataStoreBuilder,
};

use boson::messaging::{
    UserProfile,
    //MessagingClient,
    Message,
    // Client,
    ClientBuilder,
    Contact,
    ConnectionListener,
    MessageListener,
    ContactListener,
    ProfileListener
};

#[derive(Parser, Debug)]
#[command(name = "Messaging")]
#[command(version = "1.0")]
#[command(about = "Boson Messaging", long_about = None)]
struct Options {
    /// The configuration file
    #[arg(short, long, value_name = "FILE")]
    config: String,

    /// Run this program in daemon mode
    #[arg(short='D', long)]
    daemonize: bool
}

#[tokio::main]
async fn main() {
    let opts = Options::parse();
    let cfg = cfg::Builder::new()
        .load(&opts.config)
        .map_err(|e| panic!("{e}"))
        .unwrap()
        .build()
        .map_err(|e| panic!("{e}"))
        .unwrap();

    let Some(ucfg) = cfg.user() else {
        eprintln!("User item is not found in config file");
        return;
    };

    let Some(dcfg) = cfg.device() else {
        eprintln!("Device item is not found in config file");
        return;
    };

    let Some(mcfg) = cfg.messaging() else {
        eprintln!("Messaging item not found in config file");
        return;
    };

    let peerid = Id::try_from(mcfg.server_peerid())
        .map_err(|e| panic!("{e}"))
        .unwrap();

    let result = Node::new(&cfg);
    if let Err(e) = result {
        eprintln!("Creating boson Node instance error: {e}");
        return;
    }

    let node = Arc::new(Mutex::new(result.unwrap()));
    node.lock().unwrap().start();

    thread::sleep(Duration::from_secs(2));

    let mut path = String::new();
    path.push_str(cfg.data_dir());
    path.push_str("/messaging");

    let mut appdata_store = AppDataStoreBuilder::new("im")
        .with_path(path.as_str())
        .with_node(&node)
        .with_peerid(&peerid)
        .build()
        .unwrap();

    if let Err(e) = appdata_store.load().await {
        eprintln!("Loading app data store error: {e}");
        node.lock().unwrap().stop();
        return;
    }

    let Some(peer) = appdata_store.service_peer() else {
        println!("Messaging peer is not found!!!, please run it later.");
        node.lock().unwrap().stop();
        return;
    };

    let Some(ni) = appdata_store.service_node() else {
        eprintln!("Node hosting the peer not found!!!");
        node.lock().unwrap().stop();
        return;
    };

    println!("Messaging Peer: {}", peer);
    println!("Messaging Node: {}", ni);

    let usk: signature::PrivateKey = match ucfg.private_key().try_into() {
        Ok(v) => v,
        Err(_) => {
            eprintln!("Failed to convert private key from hex format");
            node.lock().unwrap().stop();
            return;
        }
    };

    let dsk: signature::PrivateKey = match dcfg.private_key().try_into() {
        Ok(v) => v,
        Err(_) => {
            eprintln!("Failed to convert device private key from hex format");
            node.lock().unwrap().stop();
            return;
        }
    };

    let user_key = signature::KeyPair::from(&usk);
    let device_key = signature::KeyPair::from(&dsk);

    let result = ClientBuilder::new()
        .with_user_key(user_key)
        .with_user_name(ucfg.name().unwrap_or("guest")).unwrap()
        .with_device_key(device_key)
        .with_device_name("test-device").unwrap()
        .with_device_node(node.clone())
        .with_app_name("test-im").unwrap()
        .with_messaging_peer(peer.clone()).unwrap()
        .with_messaging_repository("test-repo")
        .with_api_url(peer.alternative_url().as_ref().unwrap()).unwrap()
        .with_registering_user(ucfg.password().map_or("secret", |v|v))
        //.with_registering_device(dcfg.password().map_or("secret", |v|v))
        .with_connection_listener(ConnectionListenerTest)
        .with_message_listener(MessageListenerTest)
        .with_contact_listener(ContactListenerTest)
        .with_profile_listener(ProfileListenerTest)
        .build()
        .await;

    let mut client = match result {
        Ok(v) => v,
        Err(e) => {
            eprintln!("Creating messaging client instance error: {{{e}}}");
            node.lock().unwrap().stop();
            return;
        }
    };

    _ = client.start();
    thread::sleep(Duration::from_secs(1));

    _ = client.connect().await;

    thread::sleep(Duration::from_secs(2));
    _ = client.stop(false).await;
    node.lock().unwrap().stop();
}

struct ConnectionListenerTest;
impl ConnectionListener for ConnectionListenerTest {
    fn on_connecting(&self) {
        println!("Connecting to messaging service...");
    }

    fn on_connected(&self) {
        println!("Connected to messaging service");
    }

    fn on_disconnected(&self) {
        println!("Disconnected from messaging service");
    }
}

struct MessageListenerTest;
impl MessageListener for MessageListenerTest {
    fn on_message(&self, message: &Message) {
        println!("Received message: {:?}", message);
    }
    fn on_sending(&self, message: &Message) {
        println!("Sending message: {:?}", message);
    }

    fn on_sent(&self, message: &Message) {
        println!("Message sent: {:?}", message);
    }

    fn on_broadcast(&self, message: &Message) {
        println!("Broadcast message: {:?}", message);
    }
}

struct ContactListenerTest;
impl ContactListener for ContactListenerTest {
    fn on_contacts_updating(&self,
        _version_id: &str,
        _contacts: Vec<Contact>
    ) {
        println!("Contacts updating!");
    }

    fn on_contacts_updated(&self,
        _base_version_id: &str,
        _new_version_id: &str,
        _contacts: Vec<Contact>
    ) {
        println!("Contacts updated");
    }

    fn on_contacts_cleared(&self) {
        println!("Contacts cleared");
    }

    fn on_contact_profile(&self,
        _contact_id: &Id,
        _profile: &Contact
    ) {
        println!("Contact profile ");
    }
}

struct ProfileListenerTest;
impl ProfileListener for ProfileListenerTest {
    fn on_user_profile_acquired(&self, _profile: &UserProfile) {
        println!("User profile acquired");
    }

    fn on_user_profile_changed(&self, _avatar: bool) {
        println!("User profile changed");
    }
}

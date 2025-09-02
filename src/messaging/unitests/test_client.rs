use std::sync::{Arc, Mutex};
use crate::{
    Id,
    signature,
    configuration as cfg,
    config::Config,
    dht::Node,
};

use crate::messaging::{
    UserProfile,
    MessagingClient,
    Message,
    Client,
    ClientBuilder,
    Contact,
    ConnectionListener,
    MessageListener,
    ContactListener,
    ProfileListener
};

use tokio::time::sleep;
use std::time::Duration;

const PEERID: &str = "G5Q4WoLh1gfyiZQ4djRPAp6DxJBoUDY22dimtN2n6hFZ";
const NODEID: &str = "HZXXs9LTfNQjrDKvvexRhuMk8TTJhYCfrHwaj3jUzuhZ";
const BASE_URL: &str = "http://155.138.245.211:8882";

#[tokio::test]
async fn test_service_ids() {
    let url = BASE_URL.parse::<url::Url>().unwrap();
    let nodeid = Id::try_from(NODEID).unwrap();
    let peerid = Id::try_from(PEERID).unwrap();

    let result = Client::service_ids(&url).await;
    assert!(result.is_ok());

    let result = ClientBuilder::service_ids(&url).await;
    assert!(result.is_ok());

    let ids = result.unwrap();
    assert_eq!(ids.peerid(), &peerid);
    assert_eq!(ids.nodeid(), &nodeid);
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

#[ignore]
#[tokio::test]
async fn test_messaging_client() {
    let cfg  = cfg().expect("Failed to load configuration");
    let node = node(&cfg).await.expect("Failed to create node");

    let Some(mcfg) = cfg.messaging() else {
        panic!("Messaging item not found in config file");
    };

    let peerid = Id::try_from(mcfg.server_peerid())
        .map_err(|e| panic!("{e}"))
        .unwrap();

    let peer = node.lock().unwrap()
        .find_peer(&peerid, Some(4), None)
        .await
        .expect("Failed to find peer")
        .pop()
        .expect("No peer found");

    let user_key = signature::KeyPair::random();
    let dev_key  = signature::KeyPair::random();
    let result = ClientBuilder::new()
        .with_user_key(user_key.clone())
        .with_user_name("test-User").unwrap()
        .with_messaging_peer(peer.clone()).unwrap()
        .with_device_key(dev_key.clone())
        .with_device_name("test-Device").unwrap()
        .with_app_name("test-App").unwrap()
        .with_api_url(peer.alternative_url().as_ref().unwrap_or(&BASE_URL)).unwrap()
        .with_user_registration("secret")
        .with_messaging_repository("test-repo")
        .with_device_node(node.clone())
        .with_connection_listener(ConnectionListenerTest)
        .with_message_listener(MessageListenerTest)
        .with_contact_listener(ContactListenerTest)
        .with_profile_listener(ProfileListenerTest)
        .build_into()
        .await;

    if let Err(e) = &result {
        eprintln!("Creating messaging client error: {{{e}}}");
    }
    assert!(result.is_ok());

    let mut client = result.unwrap();
    let userid = Id::from(user_key.public_key());
    assert_eq!(client.userid(), &userid);

    let result = client.start().await;
    if let Err(e) = &result {
        eprintln!("Starting messaging client error: {{{e}}}");
    }
    assert!(result.is_ok());

    let result = client.connect().await;
    if let Err(e) = &result {
        eprintln!("Connecting messaging server error: {{{e}}}");
    }
    assert!(result.is_ok());
    println!(">>>>> finished connecting");
    sleep(Duration::from_secs(2)).await;

    client.stop(true).await;
    node.lock().unwrap().stop();
    crate::remove_working_path(".test-client");
}

fn cfg() -> Option<Box<dyn Config>> {
    cfg::Builder::new()
            .load_json(r#"{"ipv4":true,"ipv6":false,"port":39013,"dataDir":".test-client","logger":{"level":"debug","logFile":"im.log"},"user":{"name":"test","password":"password","privateKey":"0xee37341d0b203a4d2616ef22ba1ee92555c228a71c26e76f12c8b6b3c91872d928097a509df05df3c95d7c1516ec03bf9d387b7c29016defb7d0f1f7a2c9227f"},"bootstraps":[{"id":"HZXXs9LTfNQjrDKvvexRhuMk8TTJhYCfrHwaj3jUzuhZ","address":"155.138.245.211","port":39001},{"id":"6o6LkHgLyD5sYyW9iN5LNRYnUoX29jiYauQ5cDjhCpWQ","address":"45.32.138.246","port":39001},{"id":"8grFdb2f6LLJajHwARvXC95y73WXEanNS1rbBAZYbC5L","address":"140.82.57.197","port":39001}],"messaging":{"serverPeerId":"G5Q4WoLh1gfyiZQ4djRPAp6DxJBoUDY22dimtN2n6hFZ"}}"#)
            .map_err(|e| panic!("{e}"))
            .unwrap()
            .build()
            .map_err(|e| panic!("{e}"))
            .ok()
}

async fn node(cfg: &Box<dyn Config>) -> Option<Arc<Mutex<Node>>> {
    let node = Node::new(cfg)
        .map_err(|e| {
            panic!("Creating boson Node instance error: {e}")
        }).ok()
        .map(|node| {
            node.start();
            node
        });

    sleep(Duration::from_secs(2)).await;
    node.map(Mutex::new).map(Arc::new)
}

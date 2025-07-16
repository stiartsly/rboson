use crate::{
    Id,
    signature,
    PeerBuilder
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
async fn test_messaing_client() {
    let nodeid = Id::random();
    let url = "http://155.138.245.211:8882";
    let peer = PeerBuilder::new(&nodeid)
        .with_port(65534)
        .with_alternative_url(Some(url))
        .build();

    let user_key = signature::KeyPair::random();
    let dev_key  = signature::KeyPair::random();
    let result = ClientBuilder::new()
        .with_user_key(user_key.clone())
        .with_user_name("test-User").unwrap()
        .with_messaging_peer(peer).unwrap()
        .with_messaging_nodeid(&nodeid)
        .with_device_key(dev_key.clone())
        .with_device_name("test-Device").unwrap()
        .with_app_name("test-App").unwrap()
        .with_api_url(&"http://155.138.245.211:8882").unwrap()
        .register_user_and_device("secret")
        .with_messaging_repository("test-repo")
        .with_connection_listener(ConnectionListenerTest)
        .with_message_listener(MessageListenerTest)
        .with_contact_listener(ContactListenerTest)
        .with_profile_listener(ProfileListenerTest)
        .build()
        .await;

    if let Err(e) = &result {
        eprintln!("Error creating messaging client: {}", e);
    }
    assert!(result.is_ok());

    let mut client = result.unwrap();
    let userid = Id::from(user_key.to_public_key());
    assert_eq!(client.userid(), &userid);

    let result = client.start().await;
    if let Err(e) = &result {
        eprintln!("Error starting messaging client: {}", e);
    }
    assert!(result.is_ok());

    let result = client.connect().await;
    if let Err(e) = &result {
        eprintln!("Error connecting messaging client: {}", e);
    }
    assert!(result.is_ok());

    sleep(Duration::from_secs(10)).await;
}

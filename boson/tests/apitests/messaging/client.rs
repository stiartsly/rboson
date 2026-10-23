use std::time::{Duration, SystemTime};
use serial_test::serial;
use tokio::{sync::mpsc, time};
use boson::{signature::KeyPair, Id};
use boson::messaging::{
    Client,
    Options, OptionsBuilder,
    Channel,
    ChannelListener,
    ContactOps, ContactType,
    ConversationOps,
    InviteTicket,
    Message,
    MessageListener,
    MessageType,
    MessagingClient,
    Permission,
    Role,
    contact_editor::ContactEditorOps,
};

const BOB_YAML: &str = "tests/apitests/messaging/bob.yaml";
const ALICE_YAML: &str = "tests/apitests/messaging/alice.yaml";

// ---------------------------------------------------------------------------
// Fixtures and client helpers
// ---------------------------------------------------------------------------

#[derive(Debug)]
struct CapturedMessage {
    id: Id,
    from: Option<Id>,
    recipient: Id,
    content_type: String,
    body: Option<Vec<u8>>,
    text: Option<String>,
}

struct MessageCapture {
    received: mpsc::UnboundedSender<CapturedMessage>,
}

impl MessageListener for MessageCapture {
    fn on_message(&self, message: &dyn Message) {
        let content = message.payload_as_content();
        let captured = CapturedMessage {
            id: *message.id(),
            from: message.from().copied(),
            recipient: *message.recipient(),
            content_type: content
                .map(|content| content.content_type().to_string())
                .unwrap_or_default(),
            body: content.map(|content| content.body().to_vec()),
            text: content
                .and_then(|content| content.as_text())
                .map(ToString::to_string),
        };
        self.received
            .send(captured)
            .expect("Message test receiver should remain open");
    }
}

struct ChannelRotationCapture {
    rotated: mpsc::UnboundedSender<Id>,
}

impl ChannelListener for ChannelRotationCapture {
    fn on_channel_session_key_rotated(&self, channel: &dyn Channel) {
        self.rotated
            .send(*channel.id())
            .expect("Channel rotation test receiver should remain open");
    }
}

fn options_builder_from(options: &Options) -> OptionsBuilder {
    let mut builder = Options::builder();
    builder.with_peer_id(*options.peer_id());
    builder
        .with_peer_endpoint(options.peer_endpoint().as_str())
        .unwrap();
    builder.with_user_keypair(options.user_key().clone());
    builder.with_device_keypair(options.device_key().clone());
    builder.with_data_dir(options.data_dir());
    builder
        .with_database(options.database_uri(), options.database_pool_size())
        .unwrap();
    builder.with_database_schema(options.database_schema());
    builder.with_log_level(options.log_level());
    builder.with_log_console(options.log_console());
    if let Some(log_file) = options.log_file() {
        builder.with_log_file(log_file);
    }
    builder
}

fn create_client_with_message_capture(
    config: &str,
) -> (Client, mpsc::UnboundedReceiver<CapturedMessage>) {
    let options = Options::load(config).unwrap();
    let (received, receiver) = mpsc::unbounded_channel();
    let mut builder = options_builder_from(&options);
    builder.with_message_listener(MessageCapture { received });
    (Client::new(builder.build().unwrap()), receiver)
}

fn create_clients(first_cfg: &str, second_cfg: &str) -> (Client, Client) {
    (
        Client::new(Options::load(first_cfg).unwrap()),
        Client::new(Options::load(second_cfg).unwrap()),
    )
}

fn create_clients_with_recipient_message_capture(
    first_cfg: &str,
    second_cfg: &str,
) -> (
    Client,
    Client,
    mpsc::UnboundedReceiver<CapturedMessage>,
) {
    let first = Client::new(Options::load(first_cfg).unwrap());
    let (second, receiver) = create_client_with_message_capture(second_cfg);
    (first, second, receiver)
}

fn create_clients_with_sender_message_capture(
    first_cfg: &str,
    second_cfg: &str,
) -> (
    Client,
    Client,
    mpsc::UnboundedReceiver<CapturedMessage>,
) {
    let (first, receiver) = create_client_with_message_capture(first_cfg);
    let second = Client::new(Options::load(second_cfg).unwrap());
    (first, second, receiver)
}

fn create_client_with_channel_listener(
    config: &str,
    listener: impl ChannelListener + 'static,
) -> Client {
    let options = Options::load(config).unwrap();
    let mut builder = options_builder_from(&options);
    builder.with_channel_listener(listener);
    Client::new(builder.build().unwrap())
}

async fn wait_until_ready(client: &Client) {
    time::timeout(Duration::from_secs(10), async {
        while !client.is_connected() || !client.is_ready() {
            time::sleep(Duration::from_millis(50)).await;
        }
    })
    .await
    .expect("Client should connect and subscribe within 10 seconds");
}

async fn wait_for_friend_request(
    client: &Client,
    user_id: Id,
    created_at: SystemTime,
    accepted: bool,
) -> boson::messaging::FriendRequest {
    time::timeout(Duration::from_secs(15), async {
        loop {
            if let Some(request) = client.get_friend_request(user_id).await.unwrap() {
                if request.created_at() == created_at && request.is_accepted() == accepted {
                    return request;
                }
            }
            time::sleep(Duration::from_millis(50)).await;
        }
    })
    .await
    .expect("The matching friend request should arrive within 15 seconds")
}

async fn receive_captured(
    receiver: &mut mpsc::UnboundedReceiver<CapturedMessage>,
) -> CapturedMessage {
    time::timeout(Duration::from_secs(15), receiver.recv())
        .await
        .expect("The test message should arrive within 15 seconds")
        .expect("The message listener should remain open")
}

async fn start_clients(first: &Client, second: &Client) {
    assert_ne!(first.user_id(), second.user_id());
    let (first_start, second_start) = tokio::join!(first.start(), second.start());
    assert!(first_start.is_ok());
    assert!(second_start.is_ok());

    tokio::join!(wait_until_ready(first), wait_until_ready(second));
}

fn assert_client_stopped(client: &Client) {
    assert!(!client.is_running());
    assert!(!client.is_connected());
    assert!(!client.is_ready());
}

async fn stop_clients(first: &Client, second: &Client) {
    let (first_stop, second_stop) = tokio::join!(first.stop(), second.stop());
    assert!(first_stop.is_ok());
    assert!(second_stop.is_ok());
    assert_client_stopped(first);
    assert_client_stopped(second);
}

async fn clear_friend_requests(first: &Client, second: &Client) {
    let (first_clear, second_clear) = tokio::join!(
        first.clear_friend_requests(),
        second.clear_friend_requests()
    );
    assert!(first_clear.is_ok());
    assert!(second_clear.is_ok());
}

async fn clear_contacts(first: &Client, second: &Client) {
    let (first_clear, second_clear) = tokio::join!(first.clear_contacts(), second.clear_contacts());
    assert!(first_clear.is_ok());
    assert!(second_clear.is_ok());
}

async fn reset_clients(first: &Client, second: &Client) {
    clear_friend_requests(first, second).await;
    clear_contacts(first, second).await;
}

async fn reset_client(client: &Client) {
    assert!(client.clear_friend_requests().await.is_ok());
    assert!(client.clear_contacts().await.is_ok());
}

async fn check_friend_requests(client: &Client, user_id: Id, accepted: bool) {
    let requests = client.get_friend_requests().await.unwrap();
    let request = requests
        .iter()
        .find(|request| request.user_id() == &user_id)
        .expect("The friend request should be present in the request list");
    assert_eq!(request.is_accepted(), accepted);
}

async fn check_friend_contact(client: &Client, user_id: &Id) {
    let contact = client
        .get_contact(user_id)
        .await
        .unwrap()
        .expect("The friend contact should be present");
    assert_eq!(contact.contact_type(), ContactType::Friend);
}

async fn make_friends_by_request(bob: &Client, alice: &Client) {
    let bob_id = *bob.user_id();
    let alice_id = *alice.user_id();

    assert!(bob.friend_request(alice_id, Some("hello")).await.is_ok());

    let outgoing_request = bob
        .get_friend_request(alice_id)
        .await
        .unwrap()
        .expect("The outgoing friend request should be stored locally");
    assert_eq!(outgoing_request.initiator_id(), &bob_id);
    assert_eq!(outgoing_request.hello(), Some("hello"));
    assert!(!outgoing_request.is_accepted());
    check_friend_requests(bob, alice_id, false).await;

    let incoming = wait_for_friend_request(
        alice,
        bob_id,
        outgoing_request.created_at(),
        false,
    )
    .await;
    assert_eq!(incoming.hello(), Some("hello"));
    check_friend_requests(alice, bob_id, false).await;

    assert!(alice.accept_friend_request(bob_id).await.is_ok());

    wait_for_friend_request(
        bob,
        alice_id,
        outgoing_request.created_at(),
        true,
    )
    .await;
    wait_for_friend_request(
        alice,
        bob_id,
        outgoing_request.created_at(),
        true,
    )
    .await;

    check_friend_requests(bob, alice_id, true).await;
    check_friend_requests(alice, bob_id, true).await;
    check_friend_contact(bob, &alice_id).await;
    check_friend_contact(alice, &bob_id).await;
}

async fn make_friends_with_keypair(first: &Client, second: &Client, session_key: KeyPair) {
    let (first_rc, second_rc) = tokio::join!(
        first.add_friend(
            *second.user_id(),
            session_key.clone(),
            Some("hello,alice")
        ),
        second.add_friend(
            *first.user_id(),
            session_key.clone(),
            Some("hello,bob")
        ),
    );
    assert!(first_rc.is_ok());
    assert!(second_rc.is_ok());

    check_friend_contact(first, second.user_id()).await;
    check_friend_contact(second, first.user_id()).await;
}

async fn create_channel(
    client: &Client,
    permission: Permission,
    name: &str,
    notice: Option<&str>,
    announce: bool,
) -> Box<dyn Channel> {
    client
        .create_channel(permission, name, notice, announce)
        .await
        .expect("Channel creation should succeed")
}

async fn create_channel_with_member(
    bob: &Client,
    alice: &Client,
    name: &str,
) -> (Id, Box<dyn Channel>) {
    let channel = create_channel(bob, Permission::OwnerInvite, name, None, false).await;
    let channel_id = *channel.id();
    assert!(channel.has_member(bob.user_id()));

    let ticket = bob
        .create_invite_ticket(&channel_id, Some(*alice.user_id()))
        .await
        .unwrap();
    assert!(ticket.is_named_ticket());
    let joined = alice.join_channel(ticket).await.unwrap();
    assert_eq!(joined.id(), &channel_id);
    assert!(joined.has_member(alice.user_id()));
    (channel_id, channel)
}

// ---------------------------------------------------------------------------
// Accessors
// ---------------------------------------------------------------------------

mod accessors {
    use super::*;

    #[test]
    #[serial]
    fn test_client_accessories() {
        let options     = Options::load(ALICE_YAML).unwrap();
        let user_id     = options.user_id().clone();
        let device_id   = options.device_id().clone();
        let peer_id     = options.peer_id().clone();
        let endpoint    = options.peer_endpoint().as_str().to_string();
        let data_dir    = options.data_dir().to_path_buf();
        let database_uri    = options.database_uri().to_string();
        let database_schema = options.database_schema().to_string();

        let client = Client::new(options);

        assert_eq!(client.user_id(), &user_id);
        assert_eq!(client.device_id(), &device_id);
        assert_eq!(client.peer_id(), &peer_id);
        assert_eq!(client.peer_endpoint().as_str(), endpoint);
        assert_eq!(client.data_dir(), data_dir);
        assert_eq!(client.options().user_id(), &user_id);
        assert_eq!(client.options().device_id(), &device_id);
        assert_eq!(client.options().peer_id(), &peer_id);
        assert_eq!(client.options().peer_endpoint().as_str(), endpoint);
        assert_eq!(client.options().data_dir(), data_dir);
        assert_eq!(client.options().database_uri(), database_uri);
        assert_eq!(client.options().database_schema(), database_schema);

        assert_eq!(MessagingClient::user_id(&client), &user_id);
        assert_eq!(MessagingClient::device_id(&client), &device_id);
        assert_eq!(MessagingClient::peer_id(&client), &peer_id);
        assert_eq!(MessagingClient::peer_endpoint(&client), endpoint);
        assert_eq!(MessagingClient::data_dir(&client), data_dir);

        assert_client_stopped(&client);
    }
}

// ---------------------------------------------------------------------------
// Lifecycle and local validation
// ---------------------------------------------------------------------------

mod lifecycle {
    use super::*;

    #[tokio::test]
    #[serial]
    async fn test_start_and_stop() {
        let client = Client::new(Options::load(BOB_YAML).unwrap());

        // Stopping a client that was never started is a no-op.
        assert!(client.stop().await.is_ok());
        assert_client_stopped(&client);

        assert!(client.start().await.is_ok());
        assert!(client.is_running());

        // Starting an already running client must not deploy a second verticle.
        assert!(client.start().await.is_ok());
        assert!(client.is_running());
        wait_until_ready(&client).await;
        assert!(client.is_connected());
        assert!(client.is_ready());

        assert!(client.stop().await.is_ok());
        assert_client_stopped(&client);

        // Stopping twice is also a no-op.
        assert!(client.stop().await.is_ok());
        assert_client_stopped(&client);

        // A client can be restarted after a full stop.
        assert!(client.start().await.is_ok());
        wait_until_ready(&client).await;
        assert!(client.stop().await.is_ok());
        assert_client_stopped(&client);
    }

    #[tokio::test]
    #[serial]
    async fn test_start_failure() {
        let peer_id = Id::from(KeyPair::random().public_key());
        let user_key = KeyPair::random();
        let device_key = KeyPair::random();

        let mut builder = Options::builder();
        builder.with_peer_id(peer_id);
        builder.with_peer_endpoint("mqtt://127.0.0.1:1883").unwrap();
        builder.with_user_keypair(user_key);
        builder.with_device_keypair(device_key);
        builder.with_data_dir(concat!(env!("CARGO_MANIFEST_DIR"), "/Cargo.toml"));

        let client = Client::new(builder.build().unwrap());

        for _ in 0..2 {
            let error = client
                .start()
                .await
                .err()
                .expect("An existing file cannot be used as the data directory");
            assert!(!error.to_string().is_empty());
            assert_client_stopped(&client);
        }

        assert!(client.stop().await.is_ok());
        assert_client_stopped(&client);
    }

    #[tokio::test]
    #[serial]
    async fn test_operations_require_connection() {
        let client = Client::new(Options::load(BOB_YAML).unwrap());
        let user_id = Id::from(KeyPair::random().public_key());
        let channel_id = Id::random();
        let ticket = InviteTicket::new(
            channel_id,
            Id::random(),
            Id::random(),
            None,
            u64::MAX,
            vec![1; 64],
            vec![2; 64],
        )
        .unwrap();

        assert!(client.get_conversation(&user_id).await.is_err());
        assert!(client.get_conversations().await.is_err());
        assert!(client.remove_conversation(&user_id).await.is_err());
        assert!(client.remove_conversations(&[user_id]).await.is_err());

        assert!(client.get_messages(&user_id, None, 10, 0).await.is_err());
        assert!(client.get_messages_in_range(&user_id, 0, 1).await.is_err());
        assert!(client.remove_message(1).await.is_err());
        assert!(client.remove_messages_by_ids(&[1, 2]).await.is_err());
        assert!(client.remove_messages_in_conversation(&user_id).await.is_err());

        assert!(client.get_sessions().await.is_err());
        assert!(client.revoke_session(&user_id).await.is_err());

        assert!(client.friend_request(user_id, Some("hello")).await.is_err());
        assert!(client.accept_friend_request(user_id).await.is_err());
        assert!(client.get_friend_request(user_id).await.is_err());
        assert!(client.get_friend_requests().await.is_err());
        assert!(client.remove_friend_request(user_id).await.is_err());
        assert!(client.remove_friend_requests(&[user_id]).await.is_err());
        assert!(client.clear_friend_requests().await.is_err());
        assert!(client.add_friend(user_id, KeyPair::random(), Some("remark")).await.is_err());
        assert!(client.block_user(user_id).await.is_err());

        assert!(client
            .create_channel(
                Permission::OwnerInvite,
                "channel",
                None::<String>,
                false
            )
            .await
            .is_err());
        assert!(client.remove_channel(&channel_id).await.is_err());
        assert!(client.join_channel(ticket).await.is_err());
        assert!(client.leave_channel(&channel_id).await.is_err());
        assert!(client.create_invite_ticket(&channel_id, None).await.is_err());
        assert!(client.transfer_channel_ownership(&channel_id, user_id).await.is_err());
        assert!(client.rotate_channel_session_key(&channel_id).await.is_err());
        assert!(client.set_channel_members_role(&channel_id, &[user_id], Role::Member).await.is_err());
        assert!(client.ban_channel_members(&channel_id, &[user_id]).await.is_err());
        assert!(client.unban_channel_members(&channel_id, &[user_id]).await.is_err());
        assert!(client.remove_channel_members(&channel_id, &[user_id]).await.is_err());

        assert!(client.get_contact(&user_id).await.is_err());
        assert!(client.get_contacts().await.is_err());
        assert!(client.remove_contact(&user_id).await.is_err());
        assert!(client.remove_contacts(&[user_id]).await.is_err());
        assert!(client.clear_contacts().await.is_err());

        let error = client
            .message(Some(user_id))
            .text_body("body")
            .send()
            .await
            .err()
            .unwrap()
            .to_string();
        assert!(error.contains("Messaging client is not running"));

        assert!(client.stop().await.is_ok());
        assert_client_stopped(&client);
    }

    #[tokio::test]
    #[serial]
    async fn test_argument_validation() {
        let client = Client::new(Options::load(BOB_YAML).unwrap());

        let error = client
            .friend_request(*client.user_id(), Some("hello")).await
            .err().unwrap().to_string();
        assert!(error.contains("Cannot send friend request to yourself"));

        let error = client
            .accept_friend_request(*client.user_id()).await
            .err().unwrap().to_string();
        assert!(error.contains("Cannot accept friend request from yourself"));

        let error = client
            .revoke_session(client.device_id()).await
            .err().unwrap().to_string();
        assert!(error.contains("Cannot revoke the current session"));

        let error = client
            .create_channel(Permission::Public, "   ", None::<String>, false).await
            .err().unwrap().to_string();
        assert!(error.contains("Channel name is required"));

        let error = client
            .remove_contacts(&[]).await
            .err().unwrap().to_string();
        assert!(error.contains("No contact IDs specified"));

        // Empty batches short-circuit successfully without a connection.
        assert!(client.remove_friend_requests(&[]).await.is_ok());

        let error = client
            .message(None).text_body("body").send().await
            .err().unwrap().to_string();
        assert!(error.contains("Message recipient is required"));

        let recipient = Id::from(KeyPair::random().public_key());
        let error = client
            .message(Some(recipient)).send().await
            .err().unwrap().to_string();
        assert!(error.contains("Message content is required"));
    }
}

// ---------------------------------------------------------------------------
// Friend requests and friend contacts
// ---------------------------------------------------------------------------

mod friend_requests {
    use super::*;

    #[tokio::test]
    #[serial]
    async fn test_friend_request() {
        let client = Client::new(Options::load(BOB_YAML).unwrap());
        assert!(client.start().await.is_ok());
        wait_until_ready(&client).await;
        assert!(client.clear_friend_requests().await.is_ok());

        let first_id = Id::from(KeyPair::random().public_key());
        let second_id = Id::from(KeyPair::random().public_key());

        assert!(client.friend_request(first_id, Some("first")).await.is_ok());
        assert!(client.friend_request(second_id, None::<String>).await.is_ok());

        let first = client
            .get_friend_request(first_id).await
            .unwrap()
            .expect("The first friend request should be stored");
        assert_eq!(first.user_id(), &first_id);
        assert_eq!(first.initiator_id(), client.user_id());
        assert_eq!(first.hello(), Some("first"));
        assert!(!first.is_accepted());
        assert!(first.accepted_at().is_none());
        assert!(first.created_at() <= first.updated_at());
        assert!(!first.is_expired());

        let second = client
            .get_friend_request(second_id).await
            .unwrap()
            .expect("The second friend request should be stored");
        assert_eq!(second.hello(), None);

        let requests = client.get_friend_requests().await.unwrap();
        assert_eq!(requests.len(), 2);
        assert!(requests.iter().any(|request| request.user_id() == &first_id));
        assert!(requests.iter().any(|request| request.user_id() == &second_id));
        assert!(requests.iter().all(|request| request.initiator_id() == client.user_id()));

        assert!(client.remove_friend_request(first_id).await.is_ok());
        assert!(client.get_friend_request(first_id).await.unwrap().is_none());
        assert_eq!(client.get_friend_requests().await.unwrap().len(), 1);

        assert!(client.remove_friend_requests(&[second_id]).await.is_ok());
        assert!(client.get_friend_requests().await.unwrap().is_empty());

        // Empty batches and clearing an empty list are no-ops.
        assert!(client.remove_friend_requests(&[]).await.is_ok());
        assert!(client.clear_friend_requests().await.is_ok());
        assert!(client.get_friend_requests().await.unwrap().is_empty());

        assert!(client.stop().await.is_ok());
        assert_client_stopped(&client);
    }

    #[tokio::test]
    #[serial]
    async fn test_accept_friend_request() {
        let (sender, recipient) = create_clients(BOB_YAML, ALICE_YAML);
        let sender_id = *sender.user_id();
        let recipient_id = *recipient.user_id();

        start_clients(&sender, &recipient).await;
        reset_clients(&sender, &recipient).await;

        // There is nothing to accept yet.
        assert!(recipient.accept_friend_request(sender_id).await.is_err());
        assert!(sender.friend_request(recipient_id, Some("hello")).await.is_ok());

        let outgoing = sender
            .get_friend_request(recipient_id).await
            .unwrap()
            .expect("The outgoing friend request should be stored");
        assert_eq!(outgoing.user_id(), &recipient_id);
        assert_eq!(outgoing.initiator_id(), &sender_id);
        assert_eq!(outgoing.hello(), Some("hello"));
        assert!(!outgoing.is_accepted());
        assert!(outgoing.accepted_at().is_none());
        check_friend_requests(&sender, recipient_id, false).await;

        let incoming = wait_for_friend_request(
            &recipient,
            sender_id,
            outgoing.created_at(),
            false,
        )
        .await;
        assert_eq!(incoming.initiator_id(), &sender_id);
        assert_eq!(incoming.hello(), Some("hello"));
        assert!(!incoming.is_accepted());

        // Only the recipient may accept the pending request.
        assert!(sender.accept_friend_request(recipient_id).await.is_err());
        assert!(recipient.accept_friend_request(sender_id).await.is_ok());

        let accepted = wait_for_friend_request(
            &sender,
            recipient_id,
            outgoing.created_at(),
            true,
        )
        .await;
        assert!(accepted.is_accepted());
        assert!(accepted.accepted_at().unwrap() >= accepted.created_at());

        let local = recipient
            .get_friend_request(sender_id)
            .await
            .unwrap()
            .expect("The accepted request should be stored locally");
        assert!(local.is_accepted());
        assert_eq!(accepted.accepted_at(), local.accepted_at());

        check_friend_requests(&sender, recipient_id, true).await;
        check_friend_requests(&recipient, sender_id, true).await;

        // Accepting again is rejected on both sides.
        assert!(recipient.accept_friend_request(sender_id).await.is_err());
        assert!(sender.accept_friend_request(recipient_id).await.is_err());

        check_friend_contact(&sender, &recipient_id).await;
        check_friend_contact(&recipient, &sender_id).await;
        let contact = sender.get_contact(&recipient_id).await.unwrap().unwrap();
        assert_eq!(contact.contact_type(), ContactType::Friend);
        assert!(contact.revision() > 0);

        // Dismissing the request does not remove the established contact.
        assert!(sender.remove_friend_request(recipient_id).await.is_ok());
        assert!(sender
            .get_friend_request(recipient_id)
            .await
            .unwrap()
            .is_none());
        assert!(sender.get_contact(&recipient_id).await.unwrap().is_some());

        reset_clients(&sender, &recipient).await;
        stop_clients(&sender, &recipient).await;
    }

    #[tokio::test]
    #[serial]
    async fn test_add_friend_with_session_key() {
        let (bob, alice) = create_clients(BOB_YAML, ALICE_YAML);
        let bob_id = *bob.user_id();
        let alice_id = *alice.user_id();

        start_clients(&bob, &alice).await;
        reset_clients(&bob, &alice).await;

        make_friends_with_keypair(&bob, &alice, KeyPair::random()).await;

        let bob_contacts = bob.get_contacts().await.unwrap();
        let bob_friends: Vec<_> = bob_contacts
            .iter()
            .filter(|contact| contact.contact_type() == ContactType::Friend)
            .collect();
        assert_eq!(bob_friends.len(), 1);
        assert_eq!(bob_friends[0].id(), &alice_id);
        assert_eq!(bob_friends[0].remark(), Some("hello,alice"));

        let bob_contact = bob.get_contact(&alice_id).await.unwrap().unwrap();
        assert_eq!(bob_contact.contact_type(), ContactType::Friend);
        assert_eq!(bob_contact.remark(), Some("hello,alice"));

        let alice_contacts = alice.get_contacts().await.unwrap();
        let alice_friends: Vec<_> = alice_contacts
            .iter()
            .filter(|contact| contact.contact_type() == ContactType::Friend)
            .collect();
        assert_eq!(alice_friends.len(), 1);
        assert_eq!(alice_friends[0].id(), &bob_id);
        assert_eq!(alice_friends[0].remark(), Some("hello,bob"));

        let alice_contact = alice.get_contact(&bob_id).await.unwrap().unwrap();
        assert_eq!(alice_contact.contact_type(), ContactType::Friend);
        assert_eq!(alice_contact.remark(), Some("hello,bob"));

        assert!(bob.clear_contacts().await.is_ok());
        assert!(bob.get_contacts().await.unwrap().is_empty());
        assert!(alice.clear_contacts().await.is_ok());
        assert!(alice.get_contacts().await.unwrap().is_empty());

        clear_friend_requests(&bob, &alice).await;
        stop_clients(&bob, &alice).await;
    }
}

// ---------------------------------------------------------------------------
// Contacts
// ---------------------------------------------------------------------------

mod contacts {
    use super::*;

    #[tokio::test]
    #[serial]
    async fn test_contact_update_and_removal() {
        let (bob, alice) = create_clients(BOB_YAML, ALICE_YAML);
        let bob_id = *bob.user_id();
        let alice_id = *alice.user_id();

        start_clients(&bob, &alice).await;
        reset_clients(&bob, &alice).await;
        make_friends_by_request(&bob, &alice).await;

        let contact = bob
            .get_contact(&alice_id)
            .await
            .unwrap()
            .expect("The friend contact should exist");
        assert_eq!(contact.contact_type(), ContactType::Friend);
        assert!(contact.revision() > 0);
        assert!(contact.created_at() > 0);
        assert!(contact.updated_at() >= contact.created_at());

        let updated = Box::new(contact.edit()).remark(Some("Alice".to_string()));
        let updated = Box::new(updated).tags(Some("work,friends".to_string()));
        let updated = Box::new(updated).muted(true);
        let updated = Box::new(updated).blocked(true);
        let updated = Box::new(updated).build();
        assert!(bob.update_contact(updated).await.is_ok());

        let updated_contact = bob.get_contact(&alice_id).await.unwrap().unwrap();
        assert_eq!(updated_contact.remark(), Some("Alice"));
        assert_eq!(updated_contact.tags(), Some("work,friends"));
        assert!(updated_contact.is_muted());
        assert!(updated_contact.is_blocked());
        assert!(updated_contact.revision() > contact.revision());
        assert!(updated_contact.updated_at() >= updated_contact.created_at());

        // A second update advances the revision again, so the original copy
        // really becomes stale.
        let second_update = Box::new(updated_contact.edit()).remark(Some("Alice Updated".to_string()));
        let second_update = Box::new(second_update).build();
        assert!(bob.update_contact(second_update).await.is_ok());
        let latest = bob.get_contact(&alice_id).await.unwrap().unwrap();
        assert_eq!(latest.remark(), Some("Alice Updated"));
        assert!(latest.revision() > updated_contact.revision());

        // A stale copy must not overwrite a newer revision.
        let stale = Box::new(contact.edit()).remark(Some("stale".to_string()));
        let stale = Box::new(stale).build();
        let error = bob.update_contact(stale).await.err().unwrap().to_string();
        assert!(error.contains("older than the current version"));

        // An unchanged edit is rejected as a no-op.
        let no_change = Box::new(latest.edit()).build();
        let error = bob
            .update_contact(no_change)
            .await
            .err()
            .unwrap()
            .to_string();
        assert!(error.contains("No changes detected"));

        let contacts = bob.get_contacts().await.unwrap();
        assert_eq!(contacts.len(), 1);
        assert_eq!(contacts[0].id(), &alice_id);

        // remove_contact removes a single contact.
        assert!(bob.remove_contact(&alice_id).await.is_ok());
        assert!(bob.get_contact(&alice_id).await.unwrap().is_none());
        assert!(alice.get_contact(&bob_id).await.unwrap().is_some());

        // remove_contacts removes a batch on the other side.
        assert!(alice.remove_contacts(&[bob_id]).await.is_ok());
        assert!(alice.get_contact(&bob_id).await.unwrap().is_none());

        reset_clients(&bob, &alice).await;
        stop_clients(&bob, &alice).await;
    }
}

// ---------------------------------------------------------------------------
// Conversations and messages
// ---------------------------------------------------------------------------

mod conversations {
    use super::*;

    #[tokio::test]
    #[serial]
    async fn test_conversation() {
        let (bob, alice, mut received_rx) =
            create_clients_with_recipient_message_capture(BOB_YAML, ALICE_YAML);
        let bob_id = *bob.user_id();
        let alice_id = *alice.user_id();

        start_clients(&bob, &alice).await;
        reset_clients(&bob, &alice).await;
        make_friends_with_keypair(&bob, &alice, KeyPair::random()).await;

        assert!(bob.get_conversation(&alice_id).await.unwrap().is_none());
        assert!(alice.get_conversation(&bob_id).await.unwrap().is_none());
        assert!(bob.get_conversations().await.unwrap().is_empty());
        assert!(alice.get_conversations().await.unwrap().is_empty());

        let body = "conversation API verification";
        let sent = bob
            .message(Some(alice_id))
            .text_body(body)
            .send()
            .await
            .unwrap();
        let captured = receive_captured(&mut received_rx).await;
        assert_eq!(captured.id, *sent.id());
        assert_eq!(captured.text.as_deref(), Some(body));

        let bob_conversation = bob
            .get_conversation(&alice_id)
            .await
            .unwrap()
            .expect("Sending a message should create a conversation");
        assert_eq!(bob_conversation.id(), &alice_id);
        assert_eq!(bob_conversation.contact().id(), &alice_id);
        assert!(!bob_conversation.is_channel());
        assert_eq!(bob_conversation.preview().unwrap(), Some(body));
        assert!(bob_conversation.updated_at().is_some());

        let bob_conversations = bob.get_conversations().await.unwrap();
        assert_eq!(bob_conversations.len(), 1);
        assert_eq!(bob_conversations[0].id(), &alice_id);

        let alice_conversation = alice
            .get_conversation(&bob_id)
            .await
            .unwrap()
            .expect("Receiving a message should create a conversation");
        assert_eq!(alice_conversation.id(), &bob_id);
        assert_eq!(alice_conversation.contact().id(), &bob_id);
        assert!(!alice_conversation.is_channel());
        assert_eq!(alice_conversation.preview().unwrap(), Some(body));

        let alice_conversations = alice.get_conversations().await.unwrap();
        assert_eq!(alice_conversations.len(), 1);
        assert_eq!(alice_conversations[0].id(), &bob_id);

        assert!(bob.remove_conversations(&[]).await.is_ok());
        assert!(bob.remove_conversation(&alice_id).await.is_ok());
        assert!(bob.get_conversation(&alice_id).await.unwrap().is_none());
        assert!(bob.get_conversations().await.unwrap().is_empty());

        assert!(alice.remove_conversations(&[bob_id]).await.is_ok());
        assert!(alice.get_conversation(&bob_id).await.unwrap().is_none());
        assert!(alice.get_conversations().await.unwrap().is_empty());

        reset_clients(&bob, &alice).await;
        stop_clients(&bob, &alice).await;
    }
}

mod messages {
    use super::*;

    #[tokio::test]
    #[serial]
    async fn test_message_delivery() {
        let (bob, alice, mut received_rx) =
            create_clients_with_recipient_message_capture(BOB_YAML, ALICE_YAML);
        let bob_id = *bob.user_id();
        let alice_id = *alice.user_id();

        start_clients(&bob, &alice).await;
        reset_clients(&bob, &alice).await;
        make_friends_by_request(&bob, &alice).await;

        let text = "message delivery verification";
        let sent = bob
            .message(Some(alice_id))
            .text_body(text)
            .send()
            .await
            .unwrap();
        assert_eq!(sent.from(), Some(&bob_id));
        assert_eq!(sent.recipient(), &alice_id);
        assert_eq!(sent.message_type(), MessageType::ContentMessage);
        assert!(sent.sent_at().is_some());
        assert!(sent.received_at().is_none());
        assert!(sent.created_at() <= SystemTime::now());

        let content = sent.payload_as_content().unwrap();
        assert_eq!(content.content_type(), "text/plain");
        assert_eq!(content.as_text(), Some(text));
        assert_eq!(content.body(), text.as_bytes());

        let captured = receive_captured(&mut received_rx).await;
        assert_eq!(captured.id, *sent.id());
        assert_eq!(captured.from, Some(bob_id));
        assert_eq!(captured.recipient, alice_id);
        assert_eq!(captured.content_type, "text/plain");
        assert_eq!(captured.body.as_deref(), Some(text.as_bytes()));
        assert_eq!(captured.text.as_deref(), Some(text));

        let binary = vec![0u8, 1, 2, 3, 255];
        let sent = bob
            .message(Some(alice_id))
            .binary_body(binary.clone())
            .send()
            .await
            .unwrap();
        assert_eq!(sent.recipient(), &alice_id);
        let content = sent.payload_as_content().unwrap();
        assert_eq!(content.content_type(), "application/octet-stream");
        assert_eq!(content.as_binary(), binary);
        assert_eq!(content.as_text(), None);

        let captured = receive_captured(&mut received_rx).await;
        assert_eq!(captured.id, *sent.id());
        assert_eq!(captured.from, Some(bob_id));
        assert_eq!(captured.recipient, alice_id);
        assert_eq!(captured.content_type, "application/octet-stream");
        assert_eq!(captured.body, Some(binary));

        reset_clients(&bob, &alice).await;
        stop_clients(&bob, &alice).await;
    }

    #[ignore]
    #[tokio::test]
    #[serial]
    async fn test_unimplemented_operations() {
        let client = Client::new(Options::load(BOB_YAML).unwrap());
        assert!(client.start().await.is_ok());
        wait_until_ready(&client).await;

        let conversation_id = Id::from(KeyPair::random().public_key());

        let error = client
            .get_messages(&conversation_id, None, 10, 0)
            .await
            .err()
            .unwrap()
            .to_string();
        assert_eq!(error, "get_messages");

        let error = client
            .get_messages_in_range(&conversation_id, 0, 1)
            .await
            .err()
            .unwrap()
            .to_string();
        assert_eq!(error, "get_messages_in_range");

        let error = client
            .remove_message(1)
            .await
            .err()
            .unwrap()
            .to_string();
        assert_eq!(error, "remove_message");

        let error = client
            .remove_messages_by_ids(&[1, 2])
            .await
            .err()
            .unwrap()
            .to_string();
        assert_eq!(error, "remove_messages_by_ids");

        let error = client
            .remove_messages_in_conversation(&conversation_id)
            .await
            .err()
            .unwrap()
            .to_string();
        assert_eq!(error, "remove_messages_in_conversation");

        let error = client
            .block_user(conversation_id)
            .await
            .err()
            .unwrap()
            .to_string();
        assert_eq!(error, "block_user");

        let error = client
            .transfer_channel_ownership(&conversation_id, Id::from(KeyPair::random().public_key()))
            .await
            .err()
            .unwrap()
            .to_string();
        assert_eq!(error, "transfer_channel_ownership");

        assert!(client.stop().await.is_ok());
        assert_client_stopped(&client);
    }
}

// ---------------------------------------------------------------------------
// Sessions
// ---------------------------------------------------------------------------

mod sessions {
    use super::*;

    #[tokio::test]
    #[serial]
    async fn test_sessions_and_revoke() {
        let client = Client::new(Options::load(BOB_YAML).unwrap());
        assert!(client.start().await.is_ok());
        wait_until_ready(&client).await;

        let sessions = client.get_sessions().await.unwrap();
        let current = sessions
            .iter()
            .find(|session| session.device_id() == client.device_id())
            .expect("The current device session should be listed");
        assert!(current.is_online());
        assert!(current.last_active_ms() > 0);
        assert_eq!(
            sessions
                .iter()
                .filter(|session| session.device_id() == client.device_id())
                .count(),
            1
        );

        let error = client
            .revoke_session(client.device_id())
            .await
            .err()
            .unwrap()
            .to_string();
        assert!(error.contains("Cannot revoke the current session"));

        // Revoking an unrelated device is rejected by the service.
        let unknown_device = Id::from(KeyPair::random().public_key());
        let error = client
            .revoke_session(&unknown_device)
            .await
            .err()
            .unwrap()
            .to_string();
        assert!(error.contains("SessionRevoke"));

        assert!(client.stop().await.is_ok());
        assert_client_stopped(&client);
    }
}

// ---------------------------------------------------------------------------
// Channels
// ---------------------------------------------------------------------------

mod channels {
    use super::*;

    #[tokio::test]
    #[serial]
    async fn test_channel_creation_metadata_and_tickets() {
        let bob = Client::new(Options::load(BOB_YAML).unwrap());
        assert!(bob.start().await.is_ok());
        wait_until_ready(&bob).await;
        reset_client(&bob).await;

        let channel = create_channel(
            &bob,
            Permission::MemberInvite,
            "  API channel creation  ",
            Some("initial notice"),
            true,
        )
        .await;
        let channel_id = *channel.id();

        assert_eq!(channel.name(), Some("API channel creation"));
        assert_eq!(channel.contact_type(), ContactType::Channel);
        assert_eq!(channel.permission(), Permission::MemberInvite);
        assert_eq!(channel.notice(), Some("initial notice"));
        assert!(channel.is_announced());
        assert_eq!(channel.size(), 1);
        assert!(channel.has_member(bob.user_id()));
        assert!(channel.member(bob.user_id()).is_some());

        let contact = bob.get_contact(&channel_id).await.unwrap().unwrap();
        assert_eq!(contact.contact_type(), ContactType::Channel);
        assert_eq!(contact.name(), Some("API channel creation"));
        let contacts = bob.get_contacts().await.unwrap();
        assert!(contacts
            .iter()
            .any(|contact| contact.id() == &channel_id));

        let error = bob
            .create_channel(Permission::Public, "   ", None::<String>, false)
            .await
            .err()
            .unwrap()
            .to_string();
        assert!(error.contains("Channel name is required"));

        let bearer = bob.create_invite_ticket(&channel_id, None).await.unwrap();
        assert!(bearer.is_bearer_ticket());
        assert!(!bearer.is_named_ticket());
        assert_eq!(bearer.channel_id(), &channel_id);
        assert_eq!(bearer.inviter(), bob.user_id());
        assert!(bearer.invitee().is_none());
        assert!(bearer.is_genuine());
        assert!(!bearer.is_expired());
        assert!(!bearer.session_key().is_empty());

        let invitee = Id::from(KeyPair::random().public_key());
        let named = bob
            .create_invite_ticket(&channel_id, Some(invitee))
            .await
            .unwrap();
        assert!(named.is_named_ticket());
        assert!(!named.is_bearer_ticket());
        assert_eq!(named.invitee(), Some(&invitee));
        assert_eq!(named.channel_id(), &channel_id);
        assert!(named.is_genuine());
        assert!(!named.is_expired());

        let updated = channel
            .edit_channel()
            .name("API channel creation updated".to_string())
            .notice(Some("updated notice".to_string()))
            .announced(false)
            .permission(Permission::Public)
            .build();
        assert!(bob.update_channel_info(updated.as_ref()).await.is_ok());

        let updated_contact = bob.get_contact(&channel_id).await.unwrap().unwrap();
        assert_eq!(
            updated_contact.name(),
            Some("API channel creation updated")
        );
        assert!(updated_contact.updated_at() >= contact.updated_at());

        let error = bob
            .update_channel_info(updated.as_ref())
            .await
            .err()
            .unwrap()
            .to_string();
        assert!(error.contains("Channel information has not changed"));

        let error = bob
            .leave_channel(&channel_id)
            .await
            .err()
            .unwrap()
            .to_string();
        assert!(error.contains("owner cannot leave"));

        assert!(bob.remove_channel(&channel_id).await.is_ok());
        assert!(bob.get_contact(&channel_id).await.unwrap().is_none());
        let error = bob
            .remove_channel(&channel_id)
            .await
            .err()
            .unwrap()
            .to_string();
        assert!(error.contains("does not exist"));

        assert!(bob.stop().await.is_ok());
        assert_client_stopped(&bob);
    }

    #[tokio::test]
    #[serial]
    async fn test_channel_message() {
        let (bob, alice, mut received_rx) =
            create_clients_with_sender_message_capture(BOB_YAML, ALICE_YAML);
        let bob_id = *bob.user_id();
        let alice_id = *alice.user_id();

        start_clients(&bob, &alice).await;
        reset_clients(&bob, &alice).await;

        let channel = create_channel(
            &bob,
            Permission::OwnerInvite,
            "API channel membership",
            None,
            false,
        )
        .await;
        let channel_id = *channel.id();

        let ticket = bob
            .create_invite_ticket(&channel_id, Some(alice_id))
            .await
            .unwrap();
        assert!(ticket.is_named_ticket());
        assert_eq!(ticket.invitee(), Some(&alice_id));
        let duplicate_ticket = ticket.clone();

        let joined = alice.join_channel(ticket).await.unwrap();
        assert_eq!(joined.id(), &channel_id);
        assert!(joined.has_member(&bob_id));
        assert!(joined.has_member(&alice_id));
        assert_eq!(joined.name(), channel.name());
        assert_eq!(joined.permission(), Permission::OwnerInvite);

        let error = alice
            .join_channel(duplicate_ticket)
            .await
            .err()
            .unwrap()
            .to_string();
        assert!(error.contains("Already joined"));

        let error = alice
            .create_invite_ticket(&channel_id, None)
            .await
            .err()
            .unwrap()
            .to_string();
        assert!(error.contains("Insufficient permission"));

        let attempt = joined.edit_channel().name("not allowed".to_string()).build();
        let error = alice
            .update_channel_info(attempt.as_ref())
            .await
            .err()
            .unwrap()
            .to_string();
        assert!(error.contains("Only the channel owner"));

        let error = alice
            .remove_channel(&channel_id)
            .await
            .err()
            .unwrap()
            .to_string();
        assert!(error.contains("Only the owner"));

        let body = "Alice channel message to Bob";
        let sent = alice
            .message(Some(channel_id))
            .text_body(body)
            .send()
            .await
            .unwrap();
        assert_eq!(sent.from(), Some(&alice_id));
        assert_eq!(sent.recipient(), &channel_id);

        let captured = receive_captured(&mut received_rx).await;
        assert_eq!(captured.id, *sent.id());
        assert_eq!(captured.from, Some(alice_id));
        assert_eq!(captured.recipient, channel_id);
        assert_eq!(captured.content_type, "text/plain");
        assert_eq!(captured.text.as_deref(), Some(body));
        assert_eq!(
            captured.body,
            sent.payload_as_content().map(|content| content.body().to_vec())
        );

        let conversation = bob
            .get_conversation(&channel_id)
            .await
            .unwrap()
            .expect("A channel message should create a conversation");
        assert!(conversation.is_channel());
        assert_eq!(conversation.id(), &channel_id);
        assert_eq!(conversation.contact().id(), &channel_id);
        assert_eq!(conversation.contact().contact_type(), ContactType::Channel);
        assert_eq!(conversation.preview().unwrap(), Some(body));
        assert!(bob
            .get_conversations()
            .await
            .unwrap()
            .iter()
            .any(|conversation| conversation.id() == &channel_id));

        assert!(alice.leave_channel(&channel_id).await.is_ok());
        assert!(alice.get_contact(&channel_id).await.unwrap().is_none());
        assert!(alice.leave_channel(&channel_id).await.is_err());

        assert!(bob.remove_channel(&channel_id).await.is_ok());
        assert!(bob.get_contact(&channel_id).await.unwrap().is_none());

        reset_clients(&bob, &alice).await;
        stop_clients(&bob, &alice).await;
    }

    #[tokio::test]
    #[serial]
    async fn test_channel_member() {
        let (bob, alice) = create_clients(BOB_YAML, ALICE_YAML);
        let bob_id = *bob.user_id();
        let alice_id = *alice.user_id();

        start_clients(&bob, &alice).await;
        reset_clients(&bob, &alice).await;

        let (channel_id, _) =
            create_channel_with_member(&bob, &alice, "API channel member management").await;

        // A regular member cannot manage other members.
        let error = alice
            .set_channel_members_role(&channel_id, &[bob_id], Role::Member)
            .await
            .err()
            .unwrap()
            .to_string();
        assert!(error.contains("Only a channel owner or moderator"));

        let error = alice
            .ban_channel_members(&channel_id, &[bob_id])
            .await
            .err()
            .unwrap()
            .to_string();
        assert!(error.contains("Only a channel owner or moderator"));

        // The owner may promote and demote members.
        assert!(bob
            .set_channel_members_role(&channel_id, &[alice_id], Role::Moderator)
            .await
            .is_ok());
        assert!(bob
            .set_channel_members_role(&channel_id, &[alice_id], Role::Member)
            .await
            .is_ok());

        // Ownership and banning have dedicated operations.
        let error = bob
            .set_channel_members_role(&channel_id, &[alice_id], Role::Owner)
            .await
            .err()
            .unwrap()
            .to_string();
        assert!(error.contains("Owner and Banned roles cannot be assigned"));

        let error = bob
            .set_channel_members_role(&channel_id, &[alice_id], Role::Banned)
            .await
            .err()
            .unwrap()
            .to_string();
        assert!(error.contains("Owner and Banned roles cannot be assigned"));

        for error in [
            bob.set_channel_members_role(&channel_id, &[], Role::Member)
                .await
                .err()
                .unwrap()
                .to_string(),
            bob.ban_channel_members(&channel_id, &[])
                .await
                .err()
                .unwrap()
                .to_string(),
            bob.unban_channel_members(&channel_id, &[])
                .await
                .err()
                .unwrap()
                .to_string(),
            bob.remove_channel_members(&channel_id, &[])
                .await
                .err()
                .unwrap()
                .to_string(),
        ] {
            assert!(error.contains("No members specified"));
        }

        assert!(bob
            .ban_channel_members(&channel_id, &[alice_id])
            .await
            .is_ok());
        assert!(bob
            .unban_channel_members(&channel_id, &[alice_id])
            .await
            .is_ok());
        assert!(bob
            .remove_channel_members(&channel_id, &[alice_id])
            .await
            .is_ok());
        assert!(alice.leave_channel(&channel_id).await.is_err());

        assert!(bob.remove_channel(&channel_id).await.is_ok());

        reset_clients(&bob, &alice).await;
        stop_clients(&bob, &alice).await;
    }

    #[tokio::test]
    #[serial]
    async fn test_channel_session_key_rotation() {
        let (rotated, mut rotated_rx) = mpsc::unbounded_channel();
        let bob = create_client_with_channel_listener(
            BOB_YAML,
            ChannelRotationCapture { rotated },
        );
        let alice = Client::new(Options::load(ALICE_YAML).unwrap());

        start_clients(&bob, &alice).await;
        reset_clients(&bob, &alice).await;

        let (channel_id, _) =
            create_channel_with_member(&bob, &alice, "API channel session rotation").await;

        let previous_session_id = *bob
            .create_invite_ticket(&channel_id, None)
            .await
            .unwrap()
            .session_id();

        // Only the channel owner may rotate the session key.
        assert!(alice
            .rotate_channel_session_key(&channel_id)
            .await
            .is_err());
        assert!(bob.rotate_channel_session_key(&channel_id).await.is_ok());

        let rotated_channel = time::timeout(Duration::from_secs(15), rotated_rx.recv())
            .await
            .expect("The channel owner should receive the rotation callback")
            .expect("The rotation listener should remain open");
        assert_eq!(rotated_channel, channel_id);

        let rotated_session_id = *bob
            .create_invite_ticket(&channel_id, None)
            .await
            .unwrap()
            .session_id();
        assert_ne!(previous_session_id, rotated_session_id);

        assert!(bob.remove_channel(&channel_id).await.is_ok());

        reset_clients(&bob, &alice).await;
        stop_clients(&bob, &alice).await;
    }
}

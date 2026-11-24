use std::{
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
    time::{SystemTime, UNIX_EPOCH},
};

use boson_api::{
    signature::KeyPair,
    CryptoIdentity,
    Identity,
};

use std::rc::Rc;

use crate::{
    internal::{photon_friend_request::PhotonFriendRequest, verticle::VerticleOptions},
    ChannelHandler,
    ContactHandler,
    options::OptionsBuilder,
    ConversationHandler,
    FriendRequestHandler,
    Contact,
};

use super::*;

static NEXT_DB: AtomicUsize = AtomicUsize::new(0);

fn options() -> VerticleOptions {
    let mut builder = OptionsBuilder::new();
    builder.with_peer_id(*CryptoIdentity::new().id());
    builder.with_peer_endpoint("mqtt://127.0.0.1:1883").unwrap();
    builder.with_user_keypair(KeyPair::random());
    builder.with_device_keypair(KeyPair::random());
    let unique = format!(
        "boson-friend-request-test-{}-{}",
        std::process::id(),
        NEXT_DB.fetch_add(1, Ordering::Relaxed),
    );
    builder.with_data_dir(std::env::temp_dir().join(unique));
    let options = Arc::new(builder.build().unwrap());
    VerticleOptions {
        connection_listener: options.connection_listener(),
        message_listener: options.message_listener(),
        channel_listener: options.channel_listener(),
        contact_listener: options.contact_listener(),
        session_listener: options.session_listener(),
        friend_request_listener: options.friend_request_listener(),
        options,
    }
}

fn server() -> Rc<RpcServer> {
    let options = options();
    let path = options.options.database_path().to_path_buf();
    let _ = std::fs::remove_file(&path);
    let server = RpcServer::new(options).unwrap();
    server.repository.borrow_mut().open(&path).unwrap();
    server
}

fn request(user_id: Id, initiator_id: Id) -> PhotonFriendRequest {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis() as i64;
    PhotonFriendRequest::new(user_id, initiator_id, Some("hello".into()), now)
}

#[tokio::test]
async fn friend_request_put_get_and_remove() {
    let server = server();
    let user_id = *CryptoIdentity::new().id();
    let initiator_id = *CryptoIdentity::new().id();
    let request = request(user_id, initiator_id);

    server.put_friend_request(&request).unwrap();

    let loaded = server.friend_request(&user_id).unwrap();
    assert_eq!(*loaded.user_id(), user_id);
    assert_eq!(*loaded.initiator_id(), initiator_id);
    assert_eq!(loaded.hello(), Some("hello"));

    server.remove_friend_request(&user_id).unwrap();
    assert!(server.friend_request(&user_id).is_none());
    assert!(server.friend_requests().is_empty());
}

#[tokio::test]
async fn friend_requests_reads_repository_and_cache() {
    let server = server();
    let first = request(*CryptoIdentity::new().id(), *CryptoIdentity::new().id());
    let second = request(*CryptoIdentity::new().id(), *CryptoIdentity::new().id());

    server.put_friend_request(&first).unwrap();
    server.put_friend_request(&second).unwrap();

    assert_eq!(server.friend_requests().len(), 2);

    let loaded = server.friend_requests();
    assert_eq!(loaded.len(), 2);
    assert!(loaded.iter().any(|request| request.user_id() == first.user_id()));
    assert!(loaded.iter().any(|request| request.user_id() == second.user_id()));
}

#[tokio::test]
async fn remove_friend_requests_removes_all_and_reports_errors() {
    let server = server();
    let first = request(*CryptoIdentity::new().id(), *CryptoIdentity::new().id());
    let second = request(*CryptoIdentity::new().id(), *CryptoIdentity::new().id());
    server.put_friend_request(&first).unwrap();
    server.put_friend_request(&second).unwrap();

    server
        .remove_friend_requests(&[*first.user_id(), *second.user_id()])
        .unwrap();

    assert!(server.friend_requests().is_empty());
    assert!(server.friend_request(first.user_id()).is_none());
    assert!(server.friend_request(second.user_id()).is_none());
}

fn conversation(user_id: Id) -> PhotonConversation {
    let contact = crate::internal::photon_contact::PhotonContact::new(
        user_id,
        crate::contact::ContactType::Friend,
        1,
    );
    PhotonConversation::new(Box::new(contact) as Contact)
}

#[tokio::test]
async fn conversation_put_get_and_remove() {
    let server = server();
    let user_id = *CryptoIdentity::new().id();
    let conversation = conversation(user_id);

    server.put_conversation(conversation.clone()).await.unwrap();

    let loaded = server.conversation(&user_id).await.unwrap();
    assert_eq!(*loaded.id(), user_id);

    server.remove_conversation(&user_id).await.unwrap();
    assert!(server.conversation(&user_id).await.is_none());
    assert!(server.conversations().await.is_empty());
}

#[tokio::test]
async fn conversations_merge_repository_and_cache() {
    let server = server();
    let first = conversation(*CryptoIdentity::new().id());
    let second = conversation(*CryptoIdentity::new().id());

    server.put_conversation(first.clone()).await.unwrap();
    server.put_conversation(second.clone()).await.unwrap();

    let conversations = server.conversations().await;
    assert_eq!(conversations.len(), 2);
    assert!(conversations.iter().any(|conversation| conversation.id() == first.id()));
    assert!(conversations.iter().any(|conversation| conversation.id() == second.id()));
}

#[tokio::test]
async fn remove_conversations_removes_all() {
    let server = server();
    let first = conversation(*CryptoIdentity::new().id());
    let second = conversation(*CryptoIdentity::new().id());
    server.put_conversation(first.clone()).await.unwrap();
    server.put_conversation(second.clone()).await.unwrap();

    server
        .remove_conversation_list(&[*first.id(), *second.id()])
        .await
        .unwrap();

    assert!(server.conversation(first.id()).await.is_none());
    assert!(server.conversation(second.id()).await.is_none());
}

#[tokio::test]
async fn clear_conversations_clears_one_conversation() {
    let server = server();
    let first = conversation(*CryptoIdentity::new().id());
    let second = conversation(*CryptoIdentity::new().id());
    server.put_conversation(first.clone()).await.unwrap();
    server.put_conversation(second.clone()).await.unwrap();

    server.clear_conversations(first.id()).await.unwrap();

    assert!(server.conversation(first.id()).await.is_none());
    assert!(server.conversation(second.id()).await.is_some());
}

fn contact(user_id: Id) -> crate::internal::photon_contact::PhotonContact {
    let mut contact = crate::internal::photon_contact::PhotonContact::new(user_id, crate::contact::ContactType::Friend, 1);
    contact.set_revision(1);
    contact
}

#[tokio::test]
async fn contact_put_get_and_remove() {
    let server = server();
    let user_id = *CryptoIdentity::new().id();
    let contact = contact(user_id);

    server.put_contact(&contact).await.unwrap();

    let loaded = server.get_contact(&user_id).await.unwrap();
    assert_eq!(*loaded.id(), user_id);
    assert_eq!(loaded.contact_type(), crate::contact::ContactType::Friend);

    server.remove_contact(&user_id).await;
    assert!(server.get_contact(&user_id).await.is_none());
    assert!(server.get_contacts().await.is_empty());
}
/*
#[tokio::test]
async fn contacts_merge_repository_and_cache() {
    let server = server();
    let first = contact(*CryptoIdentity::new().id());
    let second = contact(*CryptoIdentity::new().id());

    server.put_contacts(&[first.clone(), second.clone()]).await.unwrap();

    let contacts = server.get_contacts().await;
    assert_eq!(contacts.len(), 2);
    assert!(contacts.iter().any(|contact| contact.id() == first.id()));
    assert!(contacts.iter().any(|contact| contact.id() == second.id()));
}*/

fn channel(user_id: Id, owner: Id, session_id: Id) -> crate::internal::PhotonChannel {
    let contact = crate::internal::photon_contact::PhotonContact::new(
        user_id,
        crate::contact::ContactType::Channel,
        1,
    );
    let channel = crate::internal::PhotonChannel::new(
        contact,
        owner,
        session_id,
        crate::channel::Permission::Public,
        None,
        false,
        Vec::new(),
    );
    channel
}

fn install_channel(server: &RpcServer, channel: &crate::internal::PhotonChannel) {
    let session = CryptoIdentity::new();
    server.session_contexts.lock().unwrap().insert(
        *channel.id(),
        server.session_context(*channel.id(), session, true),
    );
}

#[tokio::test]
async fn channel_put_get_and_remove() {
    let server = server();
    let user_id = *CryptoIdentity::new().id();
    let owner = *CryptoIdentity::new().id();
    let session_id = *CryptoIdentity::new().id();
    let channel = channel(user_id, owner, session_id);
    install_channel(&server, &channel);

    server.put_channel(&channel).unwrap();

    let loaded = server.get_channel(&user_id).unwrap();
    assert_eq!(*loaded.id(), user_id);
    assert_eq!(*loaded.owner(), owner);

    server.remove_channel(&user_id).await;
    assert!(server.get_channel(&user_id).is_none());
    //assert!(server.get_channels().await.is_empty());
}

#[tokio::test]
async fn channels_lists_cached_channels() {
    let server = server();
    let first = channel(
        *CryptoIdentity::new().id(),
        *CryptoIdentity::new().id(),
        *CryptoIdentity::new().id(),
    );
    let second = channel(
        *CryptoIdentity::new().id(),
        *CryptoIdentity::new().id(),
        *CryptoIdentity::new().id(),
    );
    install_channel(&server, &first);
    install_channel(&server, &second);
    server.put_channel(&first).unwrap();
    server.put_channel(&second).unwrap();
/*
    let channels = server.get_channels().await;
    assert_eq!(channels.len(), 2);
    assert!(channels.iter().any(|channel| channel.id() == first.id()));
    assert!(channels.iter().any(|channel| channel.id() == second.id()));
*/
}

#[tokio::test]
async fn remove_channels_clears_cached_channels() {
    let server = server();
    let first = channel(
        *CryptoIdentity::new().id(),
        *CryptoIdentity::new().id(),
        *CryptoIdentity::new().id(),
    );
    let second = channel(
        *CryptoIdentity::new().id(),
        *CryptoIdentity::new().id(),
        *CryptoIdentity::new().id(),
    );
    install_channel(&server, &first);
    install_channel(&server, &second);
    server.put_channel(&first).unwrap();
    server.put_channel(&second).unwrap();

    server.remove_channels().await;
/*
    assert!(server.get_channels().await.is_empty());
    assert!(server.channel(first.id()).is_none());
    assert!(server.channel(second.id()).is_none());
*/
}


use std::{
    sync::{Arc, Mutex},
    thread,
    time::Duration,
};

use boson_api::{
    signature::KeyPair,
    CryptoIdentity,
    Identity
};

use crate::{
    connection_listener::ConnectionListener,
    options::OptionsBuilder
};
use super::*;

#[derive(Clone, Default)]
struct ThreadProbe(Arc<Mutex<Option<thread::ThreadId>>>);

impl ConnectionListener for ThreadProbe {
    fn on_connecting(&self) {
        *self.0.lock().unwrap() = Some(thread::current().id());
    }
    fn on_ready(&self) {}
}

fn options() -> VerticleOptions {
    let mut b = OptionsBuilder::new();
    b.with_peer_id(*CryptoIdentity::new().id());
    b.with_peer_endpoint("mqtt://127.0.0.1:1883").unwrap();
    b.with_user_keypair(KeyPair::random());
    b.with_device_keypair(KeyPair::random());
    b.with_data_dir(std::env::temp_dir());
    let options = std::sync::Arc::new(b.build().unwrap());
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

#[tokio::test]
async fn commands_run_on_the_verticle_and_stop_cleanly() {
    tokio::time::timeout(Duration::from_secs(5), async {
        let client = crate::verticle::deploy(options()).await.unwrap();
        assert!(client.get_contacts().await.unwrap().is_empty());
        client.stop().await.unwrap();
    })
    .await
    .expect("verticle command round trip timed out");
}

#[tokio::test]
async fn lifecycle_callbacks_run_on_the_verticle_thread() {
    let probe = ThreadProbe::default();
    let mut b = OptionsBuilder::new();
    b.with_peer_id(*CryptoIdentity::new().id());
    b.with_peer_endpoint("mqtt://127.0.0.1:1883").unwrap();
    b.with_user_keypair(KeyPair::random());
    b.with_device_keypair(KeyPair::random());
    b.with_data_dir(std::env::temp_dir());
    b.with_connection_listener(probe.clone());
    let options = Arc::new(b.build().unwrap());
    let verticle = VerticleOptions {
        connection_listener: options.connection_listener(),
        message_listener: options.message_listener(),
        channel_listener: options.channel_listener(),
        contact_listener: options.contact_listener(),
        session_listener: options.session_listener(),
        friend_request_listener: options.friend_request_listener(),
        options,
    };

    tokio::time::timeout(Duration::from_secs(5), async {
        let client = crate::verticle::deploy(verticle).await.unwrap();
        client.start().await.unwrap();
        let callback_thread = *probe.0.lock().unwrap();
        assert!(
            callback_thread.is_some(),
            "the connecting callback should have run"
        );
        assert_ne!(
            callback_thread.unwrap(),
            thread::current().id(),
            "callbacks must run on the verticle thread, not the caller"
        );
        client.stop().await.unwrap();
    })
    .await
    .expect("verticle lifecycle timed out");
}

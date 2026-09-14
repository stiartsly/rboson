use crate::director::{DirectorClient, UserRegistration};
use crate::{signature::KeyPair, Id};
use base64::Engine;
use serde_json::{json, Value};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
};

#[tokio::test]
async fn test_register_user() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let user_key = KeyPair::random();
    let expected_user_key = user_key.clone();
    let expected_user_id = Id::from(user_key.public_key());
    let node_id = Id::random();
    let server = tokio::spawn(async move {
        let (mut stream, request) = accept_request(&listener).await;
        assert!(request.starts_with("GET /api/v1/client/id HTTP/1.1"));
        respond(&mut stream, &json!({ "id": node_id.to_base58() })).await;

        let (mut stream, request) = accept_request(&listener).await;
        assert!(request.starts_with("GET /api/v1/client/users/challenge HTTP/1.1"));
        respond(
            &mut stream,
            &json!({
                "challenge": base64::engine::general_purpose::URL_SAFE_NO_PAD.encode([1, 2, 3]),
                "challengeSig": base64::engine::general_purpose::URL_SAFE_NO_PAD.encode([4, 5, 6]),
                "nonce": base64::engine::general_purpose::URL_SAFE_NO_PAD.encode([7; 32]),
                "n": 6,
                "k": 2,
                "effort": 0,
            }),
        )
        .await;

        let (mut stream, request) = accept_request(&listener).await;
        assert!(request.starts_with("POST /api/v1/client/users HTTP/1.1"));
        let body = request.split("\r\n\r\n").nth(1).unwrap();
        let body: Value = serde_json::from_str(body).unwrap();
        assert_eq!(body["userId"], expected_user_id.to_base58());
        assert_eq!(body["userName"], "Alice");
        assert_eq!(body["email"], "alice@example.com");
        assert_eq!(body["bio"], "Boson user");
        assert_eq!(body["passphrase"], "secret");
        assert_eq!(body["solution"].as_array().unwrap().len(), 4);
        let user_sig = body["userSig"]
            .as_str()
            .filter(|value| !value.is_empty())
            .unwrap();
        let pow_nonce = base64::engine::general_purpose::URL_SAFE_NO_PAD
            .decode(body["powNonce"].as_str().unwrap())
            .unwrap();
        let mut signing_message = Vec::new();
        signing_message.extend_from_slice(node_id.as_bytes());
        signing_message.extend_from_slice(expected_user_key.public_key().as_bytes());
        signing_message.extend_from_slice(&[7; 32]);
        signing_message.extend_from_slice(&pow_nonce);
        signing_message.extend_from_slice(&0u32.to_be_bytes());
        assert!(expected_user_key
            .public_key()
            .verify(
                &signing_message,
                &base64::engine::general_purpose::URL_SAFE_NO_PAD
                    .decode(user_sig)
                    .unwrap(),
            )
            .unwrap());
        respond(&mut stream, &json!({ "token": "new-access-token" })).await;

        let (mut stream, request) = accept_request(&listener).await;
        assert!(request.starts_with("GET /api/v1/client/devices HTTP/1.1"));
        assert!(request.contains("authorization: Bearer new-access-token"));
        respond(&mut stream, &json!([])).await;
    });

    let mut builder = DirectorClient::builder();
    builder
        .with_director_url(format!("http://{address}"))
        .unwrap()
        .with_user_key(user_key);
    let client = builder.build().unwrap();
    client
        .register_user(
            UserRegistration::new()
                .with_name("Alice")
                .with_email("alice@example.com")
                .with_bio("Boson user")
                .with_passphrase("secret"),
        )
        .await
        .unwrap();
    assert!(client.list_devices().await.unwrap().is_empty());
    server.await.unwrap();
}

#[tokio::test]
async fn test_register_device_path() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let user_key = KeyPair::random();
    let device_key = KeyPair::random();
    let server = tokio::spawn(async move {
        let (mut stream, request) = accept_request(&listener).await;
        assert!(request.starts_with("POST /api/v1/client/auth HTTP/1.1"));
        respond(&mut stream, &json!({ "token": "device-test-token" })).await;

        let (mut stream, request) = accept_request(&listener).await;
        assert!(
            request.starts_with("POST /api/v1/client/devices HTTP/1.1"),
            "Expected path to start with /api/v1/client/devices, but got: {}",
            request.lines().next().unwrap_or_default()
        );
        assert!(request.contains("authorization: Bearer"));
        respond(&mut stream, &json!({})).await;
    });

    let client = DirectorClient::builder()
        .with_director_url(format!("http://{address}"))
        .unwrap()
        .with_user_key(user_key)
        .with_device_key(device_key)
        .build()
        .unwrap();

    client
        .register_device("Laptop", "BosonApp", None)
        .await
        .unwrap();
    server.await.unwrap();
}

#[tokio::test]
async fn test_url_prefix_deduplication() {
    let user_key = KeyPair::random();
    let client1 = DirectorClient::builder()
        .with_director_url("https://director.example.com")
        .unwrap()
        .with_user_key(user_key.clone())
        .build()
        .unwrap();
    assert_eq!(
        client1.director_url().as_str(),
        "https://director.example.com/api/v1/client/"
    );

    let client2 = DirectorClient::builder()
        .with_director_url("https://director.example.com/api/v1/client")
        .unwrap()
        .with_user_key(user_key.clone())
        .build()
        .unwrap();
    assert_eq!(
        client2.director_url().as_str(),
        "https://director.example.com/api/v1/client/"
    );

    let client3 = DirectorClient::builder()
        .with_director_url("https://director.example.com/api/v1/client/")
        .unwrap()
        .with_user_key(user_key)
        .build()
        .unwrap();
    assert_eq!(
        client3.director_url().as_str(),
        "https://director.example.com/api/v1/client/"
    );
}

#[tokio::test]
async fn test_client_close_state() {
    let client = DirectorClient::builder()
        .with_director_url("https://director.example.com")
        .unwrap()
        .build()
        .unwrap();

    assert!(!client.is_closed());
    client.close();
    assert!(client.is_closed());

    let result = client.get_node_id().await;
    assert!(result.is_err());
    assert!(result.unwrap_err().downcast_ref::<crate::errors::StateError>().is_some());
}

#[tokio::test]
async fn test_get_avatar_none() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let user_key = KeyPair::random();
    let server = tokio::spawn(async move {
        let (mut stream, _) = accept_request(&listener).await;
        respond(&mut stream, &json!({ "token": "avatar-token" })).await;

        let (mut stream, request) = accept_request(&listener).await;
        assert!(request.starts_with("GET /api/v1/client/avatar HTTP/1.1"));
        stream
            .write_all(b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n")
            .await
            .unwrap();
    });

    let client = DirectorClient::builder()
        .with_director_url(format!("http://{address}"))
        .unwrap()
        .with_user_key(user_key)
        .build()
        .unwrap();

    let avatar = client.get_avatar().await.unwrap();
    assert!(avatar.is_none());
    server.await.unwrap();
}

#[tokio::test]
async fn test_json_error_message_parsing() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let user_key = KeyPair::random();
    let server = tokio::spawn(async move {
        let (mut stream, _) = accept_request(&listener).await;
        let err_body =
            serde_json::to_vec(&json!({ "error": "Invalid signature credentials" })).unwrap();
        stream
            .write_all(
                format!(
                    "HTTP/1.1 400 Bad Request\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    err_body.len()
                )
                .as_bytes(),
            )
            .await
            .unwrap();
        stream.write_all(&err_body).await.unwrap();
    });

    let client = DirectorClient::builder()
        .with_director_url(format!("http://{address}"))
        .unwrap()
        .with_user_key(user_key)
        .build()
        .unwrap();

    let result = client.get_profile().await;
    let err = result.unwrap_err();
    assert!(err.to_string().contains("Invalid signature credentials"));
    assert!(!err.to_string().contains("{\"error\":"));
    server.await.unwrap();
}

async fn accept_request(listener: &TcpListener) -> (TcpStream, String) {
    let (mut stream, _) = listener.accept().await.unwrap();
    let mut request = Vec::new();
    loop {
        let mut buffer = [0; 1024];
        let read = stream.read(&mut buffer).await.unwrap();
        request.extend_from_slice(&buffer[..read]);
        let Some(headers_end) = request.windows(4).position(|window| window == b"\r\n\r\n") else {
            continue;
        };
        let headers = std::str::from_utf8(&request[..headers_end]).unwrap();
        let content_length = headers
            .lines()
            .find_map(|line| line.strip_prefix("content-length:"))
            .or_else(|| {
                headers
                    .lines()
                    .find_map(|line| line.strip_prefix("Content-Length:"))
            })
            .map(|value| value.trim().parse::<usize>().unwrap())
            .unwrap_or_default();
        if request.len() >= headers_end + 4 + content_length {
            return (stream, String::from_utf8(request).unwrap());
        }
    }
}

async fn respond(stream: &mut TcpStream, body: &Value) {
    let body = serde_json::to_vec(body).unwrap();
    stream
        .write_all(
            format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                body.len()
            )
            .as_bytes(),
        )
        .await
        .unwrap();
    stream.write_all(&body).await.unwrap();
}

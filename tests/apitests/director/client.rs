use boson::director::{DirectorClient, UserRegistration};
use boson::{signature::KeyPair, Id, Result};
use std::net::SocketAddr;
use std::time::Duration;
use tokio::net::{TcpListener, TcpStream};

const DEFAULT_DIRECTOR_SCHEME: &str = "https";
const DEFAULT_DIRECTOR_IP: &str = "47.101.142.224";
const DEFAULT_DIRECTOR_PORT: u16 = 9000;
const DEFAULT_DIRECTOR_NODEID: &str = "GhVW54uEd179PzRPpaiENKZuMezMNExTP6bXRK3rLDAQ";

const DEFAULT_USER_ID: &str = "4WF77gvegeWyeGProxCxX2V1o996vneixdnewuE2XUpg";
const DEFAULT_USER_KEY: &str = "0xbc12dc1054f83fcf0eba7720b706b369b58069c7e3b453be8d1f5493f69d72d13410da72883c6da7a6be00c2e5d70bbbf9a90c10bb099d01a656af2955e4af79";

fn get_director_url() -> String {
    format!(
        "{}://{}:{}",
        DEFAULT_DIRECTOR_SCHEME, DEFAULT_DIRECTOR_IP, DEFAULT_DIRECTOR_PORT
    )
}

#[tokio::test]
async fn test_fetch_node_status() {
    let url = get_director_url();

    let client = DirectorClient::builder()
        .with_director_url(url)
        .expect("Failed to set director URL")
        .with_insecure(true)
        .build()
        .expect("Failed to build DirectorClient");

    let node_id = client.get_node_id().await.expect("Failed to fetch node ID");
    println!("Successfully fetched node ID: {}", node_id.to_base58());
    assert_eq!(node_id.to_base58(), DEFAULT_DIRECTOR_NODEID);

    let status = client
        .get_node_status()
        .await
        .expect("Failed to fetch node status");
    println!("status: {}", status);
    assert_eq!(status.node_id(), &node_id);
    assert_eq!(status.software(), Some("Boson Director"));
}

#[tokio::test]
async fn test_login_and_fetch_profile() -> Result<()> {
    let user_key = DEFAULT_USER_KEY
        .parse::<KeyPair>()
        .expect("Failed to parse user key");
    let user_id = Id::from(user_key.public_key());
    assert_eq!(user_id.to_base58(), DEFAULT_USER_ID);

    let client = DirectorClient::builder()
        .with_director_url(get_director_url())?
        .with_user_key(user_key)
        .with_insecure(true)
        .build()?;

    let profile = client.get_profile().await?;
    println!("Successfully fetched profile: {profile}");
    assert_eq!(profile.id(), user_id);

    Ok(())
}

#[ignore]
#[tokio::test]
async fn test_register_new_user() -> Result<()> {
    let url = get_director_url();
    let user_key = KeyPair::random();
    let device_key = KeyPair::random();

    let client = DirectorClient::builder()
        .with_director_url(&url)?
        .with_user_key(user_key.clone())
        .with_device_key(device_key)
        .with_insecure(true)
        .build()?;

    let user_name = format!("user_{:08x}", rand::random::<u32>());
    let email = format!("{user_name}@example.com");
    let mut registration = UserRegistration::new()
        .with_name(user_name.clone())
        .with_email(email)
        .with_bio("Registered via DirectorClient apitest")
        .with_initial_device("APITestDevice", "BosonAPITest");

    let passphrase = Some("test_secret_123");
    if let Some(pass) = passphrase {
        registration = registration.with_passphrase(pass);
    }

    client.register_user(registration).await?;

    // Verify registration succeeded and authenticated calls work
    let profile = client.get_profile().await?;
    assert_eq!(profile.name().map(|s| s.as_str()), Some(user_name.as_str()));
    assert_eq!(profile.id(), Id::from(user_key.public_key()));

    let devices = client.list_devices().await?;
    assert!(
        !devices.is_empty(),
        "Expected initial device to be present in devices list"
    );

    Ok(())
}

#[tokio::test]
async fn test_register_user_given_target_address() -> Result<()> {
    use base64::Engine;
    use serde_json::{json, Value};
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let addr = listener.local_addr()?;

    let node_id = Id::random();

    let server = tokio::spawn(async move {
        // 1. GET /api/v1/client/id
        let (mut stream, request) = accept_http_request(&listener).await;
        assert!(request.starts_with("GET /api/v1/client/id HTTP/1.1"));
        send_json_response(&mut stream, &json!({ "id": node_id.to_base58() })).await;

        // 2. GET /api/v1/client/users/challenge
        let (mut stream, request) = accept_http_request(&listener).await;
        assert!(request.starts_with("GET /api/v1/client/users/challenge HTTP/1.1"));
        send_json_response(
            &mut stream,
            &json!({
                "challenge": base64::engine::general_purpose::URL_SAFE_NO_PAD.encode([1, 2, 3]),
                "challengeSig": base64::engine::general_purpose::URL_SAFE_NO_PAD.encode([4, 5, 6]),
                "nonce": base64::engine::general_purpose::URL_SAFE_NO_PAD.encode([9; 32]),
                "n": 6,
                "k": 2,
                "effort": 0,
            }),
        )
        .await;

        // 3. POST /api/v1/client/usersAndInitialDevice
        let (mut stream, request) = accept_http_request(&listener).await;
        assert!(request.starts_with("POST /api/v1/client/usersAndInitialDevice HTTP/1.1"));
        let body_str = request.split("\r\n\r\n").nth(1).unwrap();
        let body: Value = serde_json::from_str(body_str).unwrap();
        assert_eq!(body["userName"], "Bob");
        assert_eq!(body["email"], "bob@example.com");
        assert_eq!(body["deviceName"], "APITestDevice");
        assert_eq!(body["appName"], "BosonAPITest");
        let registered_user_id = body["userId"].as_str().unwrap().to_string();
        let registered_device_id = body["deviceId"].as_str().unwrap().to_string();
        send_json_response(
            &mut stream,
            &json!({ "token": "jwt-director-apitest-token" }),
        )
        .await;

        // 4. GET /api/v1/client/profile
        let (mut stream, request) = accept_http_request(&listener).await;
        assert!(request.starts_with("GET /api/v1/client/profile HTTP/1.1"));
        send_json_response(
            &mut stream,
            &json!({
                "id": registered_user_id,
                "name": "Bob",
                "email": "bob@example.com",
                "bio": "Registered via DirectorClient apitest",
                "admin": false,
                "createdAt": 1700000000,
                "updatedAt": 1700000000,
                "planName": "free",
                "passphraseProtected": true,
            }),
        )
        .await;

        // 5. GET /api/v1/client/devices
        let (mut stream, request) = accept_http_request(&listener).await;
        assert!(request.starts_with("GET /api/v1/client/devices HTTP/1.1"));
        send_json_response(
            &mut stream,
            &json!([
                {
                    "id": registered_device_id,
                    "userId": registered_user_id,
                    "name": "APITestDevice",
                    "app": "BosonAPITest",
                    "createdAt": 1700000000,
                    "updatedAt": 1700000000,
                    "lastSeen": 1700000000,
                }
            ]),
        )
        .await;
    });

    let (client, user_key) =
        register_user_on_director(addr, "Bob", "bob@example.com", Some("pass123")).await?;

    let profile = client.get_profile().await?;
    assert_eq!(profile.name().map(|s| s.as_str()), Some("Bob"));
    assert_eq!(profile.id(), Id::from(user_key.public_key()));

    let devices = client.list_devices().await?;
    assert_eq!(devices.len(), 1);
    assert_eq!(devices[0].name(), Some("APITestDevice"));
    assert_eq!(devices[0].app(), Some("BosonAPITest"));

    server.await.unwrap();
    Ok(())
}

async fn register_user_on_director(
    address: SocketAddr,
    user_name: &str,
    email: &str,
    passphrase: Option<&str>,
) -> Result<(DirectorClient, KeyPair)> {
    let url = format!("http://{address}");
    let user_key = KeyPair::random();
    let device_key = KeyPair::random();

    let client = DirectorClient::builder()
        .with_director_url(&url)?
        .with_user_key(user_key.clone())
        .with_device_key(device_key)
        .with_insecure(true)
        .build()?;

    let mut registration = UserRegistration::new()
        .with_name(user_name)
        .with_email(email)
        .with_bio("Registered via DirectorClient apitest")
        .with_initial_device("APITestDevice", "BosonAPITest");

    if let Some(pass) = passphrase {
        registration = registration.with_passphrase(pass);
    }

    client.register_user(registration).await?;
    Ok((client, user_key))
}

async fn accept_http_request(listener: &TcpListener) -> (TcpStream, String) {
    use tokio::io::AsyncReadExt;

    let (mut stream, _) = tokio::time::timeout(Duration::from_secs(10), listener.accept())
        .await
        .expect("Timed out waiting for an HTTP request")
        .expect("Failed to accept an HTTP request");
    let mut request = Vec::new();
    loop {
        let mut buffer = [0; 1024];
        let read = stream.read(&mut buffer).await.unwrap();
        assert_ne!(read, 0, "Connection closed before a complete HTTP request");
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

async fn send_json_response(stream: &mut TcpStream, body: &serde_json::Value) {
    use tokio::io::AsyncWriteExt;

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

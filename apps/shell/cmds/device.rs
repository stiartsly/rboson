use std::{
    collections::{BTreeMap, HashMap},
    path::{Path, PathBuf},
};

use boson::{
    director::Client,
    signature::{KeyPair, PrivateKey},
    Id,
};
use clap::{Arg, ArgMatches, Command};
use serde::{Deserialize, Serialize};

use super::parse_id;

#[derive(Serialize, Deserialize)]
#[serde(untagged)]
enum StoredKey {
    Hex(String),
    Record { private_key: String },
}

pub(crate) struct DeviceStore {
    file_path: PathBuf,
    devices: HashMap<Id, PrivateKey>,
}

impl DeviceStore {
    pub(crate) fn load(data_dir: &Path) -> Self {
        let file_path = data_dir.join("devices.json");
        let mut devices = HashMap::new();
        if file_path.exists() {
            if let Ok(content) = std::fs::read_to_string(&file_path) {
                if let Ok(entries) = serde_json::from_str::<HashMap<String, StoredKey>>(&content) {
                    for (k, v) in entries {
                        let key_str = match v {
                            StoredKey::Hex(s) => s,
                            StoredKey::Record { private_key } => private_key,
                        };
                        if let (Ok(id), Ok(pk)) = (
                            Id::try_from(k.as_str()),
                            PrivateKey::try_from(key_str.as_str()),
                        ) {
                            devices.insert(id, pk);
                        }
                    }
                }
            }
        }
        Self { file_path, devices }
    }

    pub(crate) fn get(&self, id: &Id) -> Option<&PrivateKey> {
        self.devices.get(id)
    }

    pub(crate) fn insert(&mut self, id: Id, key: PrivateKey) -> std::io::Result<()> {
        self.devices.insert(id, key);
        self.save()
    }

    pub(crate) fn list_ids(&self) -> Vec<Id> {
        let mut ids: Vec<Id> = self.devices.keys().copied().collect();
        ids.sort();
        ids
    }

    pub(crate) fn save(&self) -> std::io::Result<()> {
        if let Some(parent) = self.file_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let records: BTreeMap<String, String> = self
            .devices
            .iter()
            .map(|(id, key)| (id.to_base58(), key.to_hexstr()))
            .collect();
        let content = serde_json::to_string_pretty(&records)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
        std::fs::write(&self.file_path, content)?;
        Ok(())
    }
}

pub(crate) fn command() -> Command {
    Command::new("device")
        .about("Manage devices and their private keys")
        .arg(
            Arg::new("list")
                .short('l')
                .long("list")
                .help("List all device IDs")
                .num_args(0..=1)
                .default_missing_value("")
                .value_name("FILTER")
                .conflicts_with("new"),
        )
        .arg(
            Arg::new("new")
                .short('n')
                .long("new")
                .help("Register a new device with optional private key (generated if omitted)")
                .num_args(0..=1)
                .default_missing_value("")
                .value_name("DEVICE_PRIVATE_KEY")
                .conflicts_with("list"),
        )
        .arg(
            Arg::new("device_id_flag")
                .long("deviceId")
                .visible_alias("device-id")
                .help("Device ID to show private key for")
                .num_args(1)
                .value_name("DEVICEID")
                .conflicts_with_all(["list", "new"]),
        )
        .arg(
            Arg::new("device_id")
                .help("Device ID to show private key for")
                .value_name("DEVICEID")
                .required(false)
                .conflicts_with_all(["list", "new"]),
        )
}

pub(crate) async fn run(matches: &ArgMatches, client: &Client, data_dir: &str) {
    let mut store = DeviceStore::load(Path::new(data_dir));

    let is_new = matches.contains_id("new")
        || matches.get_one::<String>("device_id").map(String::as_str) == Some("new");

    let is_list = matches.contains_id("list")
        || matches.get_one::<String>("device_id").map(String::as_str) == Some("list");

    if is_new {
        register_new_device(matches, client, &mut store).await;
    } else if is_list {
        list_devices(client, &store).await;
    } else if let Some(id_str) = matches
        .get_one::<String>("device_id")
        .or_else(|| matches.get_one::<String>("device_id_flag"))
    {
        show_device_key(id_str, &store);
    } else {
        println!("Usage:");
        println!("  device --list                   List all device IDs");
        println!("  device <DEVICEID>               Show the private key for specified device ID");
        println!("  device --new [PRIVATE_KEY]      Register a new device with private key (generated if omitted)");
    }
}

async fn register_new_device(matches: &ArgMatches, client: &Client, store: &mut DeviceStore) {
    let key_arg = matches
        .get_one::<String>("new")
        .map(|s| s.trim())
        .filter(|s| !s.is_empty() && *s != "[]");

    let keypair = match key_arg {
        Some(key_str) => match PrivateKey::try_from(key_str) {
            Ok(pk) => KeyPair::from(&pk),
            Err(e) => {
                println!("\x1b[31mInvalid private key '{key_str}': {e}\x1b[0m");
                return;
            }
        },
        None => KeyPair::random(),
    };

    let device_id = Id::from(keypair.public_key());
    println!(
        "Registering device {} with the Director...",
        device_id.to_base58()
    );

    let passphrase = client.options().registration().and_then(|r| r.passphrase());
    let reg_result = client
        .register_device_with_key(&keypair, "Boson Shell Device", "Boson Shell", passphrase)
        .await;

    let reg_result = match reg_result {
        Err(e) if passphrase.is_some() => {
            // Retry without passphrase if rejected
            client
                .register_device_with_key(&keypair, "Boson Shell Device", "Boson Shell", None)
                .await
                .or(Err(e))
        }
        other => other,
    };

    match reg_result {
        Ok(()) => {
            if let Err(e) = store.insert(device_id, keypair.private_key().clone()) {
                println!(
                    "\x1b[33mWarning: Registered device but failed to save key locally: {e}\x1b[0m"
                );
            }
            println!("Device registered successfully.");
            println!("  Device ID   : {}", device_id.to_base58());
            println!(
                "  Private Key : {} (base58)",
                keypair.private_key().to_base58()
            );
            println!(
                "              : {} (hex)",
                keypair.private_key().to_hexstr()
            );
        }
        Err(e) => {
            println!("\x1b[31mFailed to register device: {e}\x1b[0m");
        }
    }
}

async fn list_devices(client: &Client, store: &DeviceStore) {
    match client.list_devices().await {
        Ok(devices) => {
            let mut ids: Vec<Id> = devices.iter().map(|d| *d.id()).collect();
            for local_id in store.list_ids() {
                if !ids.contains(&local_id) {
                    ids.push(local_id);
                }
            }
            if ids.is_empty() {
                println!("No devices found.");
            } else {
                println!("Device IDs ({}):", ids.len());
                for id in &ids {
                    println!("  {}", id.to_base58());
                }
            }
        }
        Err(e) => {
            let local_ids = store.list_ids();
            if !local_ids.is_empty() {
                println!("Device IDs ({}, offline):", local_ids.len());
                for id in &local_ids {
                    println!("  {}", id.to_base58());
                }
            } else {
                println!("\x1b[31mUnable to list devices from Director: {e}\x1b[0m");
            }
        }
    }
}

fn show_device_key(device_id_str: &str, store: &DeviceStore) {
    let Some(id) = parse_id(device_id_str) else {
        return;
    };
    if let Some(private_key) = store.get(&id) {
        println!("Device ID   : {}", id.to_base58());
        println!(
            "Private Key : {} (base58)",
            private_key.to_base58()
        );
        println!(
            "            : {} (hex)",
            private_key.to_hexstr()
        );
    } else {
        println!(
            "\x1b[31mNo private key found for device ID '{}'.\x1b[0m",
            id.to_base58()
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_device_store_insert_get_save_load() {
        let temp_dir = std::env::temp_dir().join(format!("boson_test_{}", Id::random().to_base58()));
        let _ = std::fs::create_dir_all(&temp_dir);

        let mut store = DeviceStore::load(&temp_dir);
        assert!(store.list_ids().is_empty());

        let keypair = KeyPair::random();
        let device_id = Id::from(keypair.public_key());
        store
            .insert(device_id, keypair.private_key().clone())
            .unwrap();

        assert_eq!(store.get(&device_id), Some(keypair.private_key()));
        assert_eq!(store.list_ids(), vec![device_id]);

        // Reload from disk
        let reloaded = DeviceStore::load(&temp_dir);
        assert_eq!(reloaded.get(&device_id), Some(keypair.private_key()));
        assert_eq!(reloaded.list_ids(), vec![device_id]);

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_device_command_parsing() {
        let cmd = command();

        // --list
        let matches = cmd.clone().try_get_matches_from(["device", "--list"]).unwrap();
        assert!(matches.contains_id("list"));

        // --list []
        let matches = cmd.clone().try_get_matches_from(["device", "--list", "[]"]).unwrap();
        assert!(matches.contains_id("list"));

        // -l
        let matches = cmd.clone().try_get_matches_from(["device", "-l"]).unwrap();
        assert!(matches.contains_id("list"));

        // device DEVICEID
        let random_id = Id::random().to_base58();
        let matches = cmd.clone().try_get_matches_from(["device", &random_id]).unwrap();
        assert_eq!(
            matches.get_one::<String>("device_id").map(String::as_str),
            Some(random_id.as_str())
        );

        // device --deviceId DEVICEID
        let matches = cmd.clone().try_get_matches_from(["device", "--deviceId", &random_id]).unwrap();
        assert_eq!(
            matches.get_one::<String>("device_id_flag").map(String::as_str),
            Some(random_id.as_str())
        );

        // --new without key
        let matches = cmd.clone().try_get_matches_from(["device", "--new"]).unwrap();
        assert!(matches.contains_id("new"));
        assert_eq!(matches.get_one::<String>("new").map(String::as_str), Some(""));

        // -n without key
        let matches = cmd.clone().try_get_matches_from(["device", "-n"]).unwrap();
        assert!(matches.contains_id("new"));

        // --new with key
        let key = KeyPair::random().private_key().to_hexstr();
        let matches = cmd.clone().try_get_matches_from(["device", "--new", &key]).unwrap();
        assert_eq!(
            matches.get_one::<String>("new").map(String::as_str),
            Some(key.as_str())
        );

        // -n with key
        let matches = cmd.clone().try_get_matches_from(["device", "-n", &key]).unwrap();
        assert_eq!(
            matches.get_one::<String>("new").map(String::as_str),
            Some(key.as_str())
        );
    }

    #[tokio::test]
    #[serial_test::serial]
    async fn test_device_run_new_and_show_and_list() {
        use boson::director::Options as DirectorOptions;
        use serde_json::json;
        use std::sync::{Arc, Mutex};
        use tokio::net::TcpListener;

        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();

        let temp_dir = std::env::temp_dir().join(format!("boson_test_run_{}", Id::random().to_base58()));
        let _ = std::fs::create_dir_all(&temp_dir);

        let user_key = KeyPair::random();
        let server_user_key = user_key.clone();
        let registered_device_id_cell = Arc::new(Mutex::new(None::<String>));
        let reg_id_clone = registered_device_id_cell.clone();

        let server = tokio::spawn(async move {
            // 1. POST /api/v1/client/auth (for register_device)
            let (mut stream, request) = accept_request(&listener).await;
            assert!(request.starts_with("POST /api/v1/client/auth HTTP/1.1"));
            respond(&mut stream, &json!({ "token": "test-auth-token" })).await;

            // 2. POST /api/v1/client/devices (device registration)
            let (mut stream, request) = accept_request(&listener).await;
            assert!(request.starts_with("POST /api/v1/client/devices HTTP/1.1"));
            let body_str = request.split("\r\n\r\n").nth(1).unwrap();
            let body: serde_json::Value = serde_json::from_str(body_str).unwrap();
            let dev_id = body["deviceId"].as_str().unwrap().to_string();
            *reg_id_clone.lock().unwrap() = Some(dev_id.clone());
            respond(&mut stream, &json!({})).await;

            // 3. GET /api/v1/client/devices (list devices)
            let (mut stream, request) = accept_request(&listener).await;
            assert!(request.starts_with("GET /api/v1/client/devices HTTP/1.1"));
            respond(
                &mut stream,
                &json!([
                    {
                        "id": dev_id,
                        "userId": Id::from(server_user_key.public_key()).to_base58(),
                        "name": "Boson Shell Device",
                        "app": "Boson Shell",
                    }
                ]),
            )
            .await;
        });

        let output_lines = Arc::new(Mutex::new(Vec::new()));
        let lines_clone = output_lines.clone();
        crate::cmds::set_result_output(move |line| {
            lines_clone.lock().unwrap().push(line);
        });

        let options = DirectorOptions::new(format!("http://{address}"))
            .unwrap()
            .with_user_id(Id::from(user_key.public_key()))
            .with_user_private_key(user_key.private_key().clone());
        let client = Client::new(options).unwrap();
        let cmd = command();

        // 1. Run device --new
        let matches = cmd.clone().try_get_matches_from(["device", "--new"]).unwrap();
        run(&matches, &client, temp_dir.to_str().unwrap()).await;

        let registered_id = registered_device_id_cell.lock().unwrap().clone().unwrap();

        let lines = output_lines.lock().unwrap().clone();
        assert!(lines.iter().any(|l| l.contains("Device registered successfully")));
        assert!(lines.iter().any(|l| l.contains(&registered_id)));

        output_lines.lock().unwrap().clear();

        // 2. Run device <DEVICEID>
        let matches = cmd.clone().try_get_matches_from(["device", &registered_id]).unwrap();
        run(&matches, &client, temp_dir.to_str().unwrap()).await;

        let lines = output_lines.lock().unwrap().clone();
        assert!(lines.iter().any(|l| l.contains(&registered_id)));
        assert!(lines.iter().any(|l| l.contains("Private Key")));

        output_lines.lock().unwrap().clear();

        // 3. Run device --list
        let matches = cmd.clone().try_get_matches_from(["device", "--list"]).unwrap();
        run(&matches, &client, temp_dir.to_str().unwrap()).await;

        let lines = output_lines.lock().unwrap().clone();
        assert!(lines.iter().any(|l| l.contains(&registered_id)));

        output_lines.lock().unwrap().clear();

        // 4. Run device with unknown ID
        let unknown_id = Id::random().to_base58();
        let matches = cmd.clone().try_get_matches_from(["device", &unknown_id]).unwrap();
        run(&matches, &client, temp_dir.to_str().unwrap()).await;

        let lines = output_lines.lock().unwrap().clone();
        assert!(lines.iter().any(|l| l.contains("No private key found")));

        server.await.unwrap();
        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[tokio::test]
    #[serial_test::serial]
    async fn test_device_run_new_with_provided_key() {
        use boson::director::Options as DirectorOptions;
        use serde_json::json;
        use std::sync::{Arc, Mutex};
        use tokio::net::TcpListener;

        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();

        let temp_dir = std::env::temp_dir().join(format!("boson_test_provided_{}", Id::random().to_base58()));
        let _ = std::fs::create_dir_all(&temp_dir);

        let user_key = KeyPair::random();
        let dev_keypair = KeyPair::random();
        let expected_dev_id = Id::from(dev_keypair.public_key()).to_base58();
        let server_dev_id = expected_dev_id.clone();

        let server = tokio::spawn(async move {
            // 1. POST /api/v1/client/auth
            let (mut stream, _) = accept_request(&listener).await;
            respond(&mut stream, &json!({ "token": "test-auth-token" })).await;

            // 2. POST /api/v1/client/devices
            let (mut stream, request) = accept_request(&listener).await;
            let body_str = request.split("\r\n\r\n").nth(1).unwrap();
            let body: serde_json::Value = serde_json::from_str(body_str).unwrap();
            assert_eq!(body["deviceId"], server_dev_id);
            respond(&mut stream, &json!({})).await;
        });

        let output_lines = Arc::new(Mutex::new(Vec::new()));
        let lines_clone = output_lines.clone();
        crate::cmds::set_result_output(move |line| {
            lines_clone.lock().unwrap().push(line);
        });

        let options = DirectorOptions::new(format!("http://{address}"))
            .unwrap()
            .with_user_id(Id::from(user_key.public_key()))
            .with_user_private_key(user_key.private_key().clone());
        let client = Client::new(options).unwrap();
        let cmd = command();

        let key_str = dev_keypair.private_key().to_hexstr();
        let matches = cmd.clone().try_get_matches_from(["device", "--new", &key_str]).unwrap();
        run(&matches, &client, temp_dir.to_str().unwrap()).await;

        let lines = output_lines.lock().unwrap().clone();
        assert!(lines.iter().any(|l| l.contains("Device registered successfully")));
        assert!(lines.iter().any(|l| l.contains(&expected_dev_id)));

        server.await.unwrap();
        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    async fn accept_request(listener: &tokio::net::TcpListener) -> (tokio::net::TcpStream, String) {
        use tokio::io::AsyncReadExt;
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
                return (stream, String::from_utf8_lossy(&request).into_owned());
            }
        }
    }

    async fn respond(stream: &mut tokio::net::TcpStream, json: &serde_json::Value) {
        use tokio::io::AsyncWriteExt;
        let body = serde_json::to_vec(json).unwrap();
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
}

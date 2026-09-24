use clap::{Parser, Subcommand};
use std::{env, time::UNIX_EPOCH};
use boson::{signature, Id, Result};
use boson::director::{self,
    Client, Device, NodeStatus, Profile,
    Service, UserRegistration
};

#[derive(Parser, Debug)]
#[command(name = "identity")]
#[command(
    about = "Boson Director and Identity management tool",
    long_about = None
)]
struct Cli {
    /// Director node URL (default: $BOSON_DIRECTOR_URL)
    #[arg(long = "director-url", visible_alias = "director_url", global = true, value_name = "URL")]
    director_url: Option<String>,

    /// Accept invalid TLS certificates from the Director endpoint
    #[arg(long, global = true)]
    insecure: bool,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Display information about a user and list their authorized devices
    #[command(name = "user")]
    User {
        /// User ID (defaults to $BOSON_USER_ID)
        #[arg(value_name = "USERID")]
        user_id: Option<String>,

        /// User private key to authenticate with Director (defaults to $BOSON_USER_PRIVATE_KEY)
        #[arg(short = 'u', long = "user-key", value_name = "PRIVATE_KEY")]
        user_key: Option<String>,

        /// List all authorized devices for this user
        #[arg(short = 'l', long = "devices", default_value_t = false)]
        devices: bool,
    },

    /// Register a new user with the Director node
    #[command(name = "reguser")]
    RegUser {
        /// User private key (defaults to $BOSON_USER_PRIVATE_KEY or generates random if omitted)
        #[arg(value_name = "PRIVATE_KEY")]
        user_key: String,

        /// User display name (random-generated with 'user_' prefix if omitted)
        #[arg(long, value_name = "STRING")]
        name: Option<String>,

        /// User email (random-generated with 'user_' prefix if omitted)
        #[arg(long, value_name = "STRING")]
        email: Option<String>,

        /// User biography (random-generated with 'Boson user' prefix if omitted)
        #[arg(long, value_name = "STRING")]
        bio: Option<String>,

        /// Passphrase for user registration
        #[arg(long, value_name = "STRING")]
        passphrase: Option<String>,
    },

    /// Register a new device under an authorized user
    #[command(name = "regdev")]
    RegDev {
        /// Device private key (generates random if omitted)
        #[arg(value_name = "DEVICE_PRIVATE_KEY")]
        device_key: String,

        /// User private key authorizing this device (defaults to $BOSON_USER_PRIVATE_KEY)
        #[arg(short = 'u', long = "user-key", value_name = "PRIVATE_KEY")]
        user_key: Option<String>,

        /// Device name (random-generated with 'device_' prefix if omitted)
        #[arg(long = "device-name", value_name = "STRING")]
        device_name: Option<String>,

        /// Application name (defaults to BosonApp if omitted)
        #[arg(long, value_name = "STRING")]
        app: Option<String>,

        /// Passphrase for device registration
        #[arg(long, value_name = "STRING")]
        passphrase: Option<String>,
    },
}

#[tokio::main(flavor = "current_thread")]
async fn main() {
    if let Err(error) = run().await {
        eprintln!("identity: {error}");
        std::process::exit(1);
    }
}

async fn run() -> Result<()> {
    let cli = Cli::parse();

    match &cli.command {
        Some(Commands::User { user_id, user_key, devices }) => {
            user_command(
                &cli,
                user_id.as_deref(),
                user_key.as_deref(),
                *devices
            )
            .await
        }
        Some(Commands::RegUser {
            user_key,
            name,
            email,
            bio,
            passphrase,
        }) => {
            reguser_command(
                &cli,
                user_key.as_str(),
                name.as_deref(),
                email.as_deref(),
                bio.as_deref(),
                passphrase.as_deref(),
            )
            .await
        }
        Some(Commands::RegDev {
            device_key,
            user_key,
            device_name,
            app,
            passphrase,
        }) => {
            regdev_command(
                &cli,
                device_key.as_str(),
                user_key.as_deref(),
                device_name.as_deref(),
                app.as_deref(),
                passphrase.as_deref(),
            )
            .await
        }
        _ => {
            director_command(&cli, None).await
        }
    }
}

fn resolve_director_url(cli: &Cli, input_url: Option<&str>) -> Result<String> {
    let url = input_url
        .map(str::to_owned)
        .or_else(|| cli.director_url.clone())
        .or_else(|| env::var("BOSON_DIRECTOR_URL").ok())
        .ok_or_else(|| {
            boson::errors::ArgumentError::new(
                "Director URL must be provided or set in $BOSON_DIRECTOR_URL",
            )
        })?;
    Ok(url)
}

async fn director_command(cli: &Cli, url: Option<&str>) -> Result<()> {
    let director_url = resolve_director_url(cli, url)?;
    let dir_opts = director::Options::new(&director_url)?
        .with_insecure(cli.insecure);
    let client = Client::new(dir_opts)?;
    let status = client.fetch_node_status().await?;
    print_status(&director_url, &status, None);
    Ok(())
}

async fn user_command(
    cli: &Cli,
    user_id_arg: Option<&str>,
    user_key_arg: Option<&str>,
    devices: bool
) -> Result<()> {
    let director_url = resolve_director_url(cli, None)?;


    let user_id_arg = user_id_arg
        .map(str::to_owned)
        .or_else(|| env::var("BOSON_USER_ID").ok())
        .map(|s| Id::try_from(s.as_str()))
        .transpose()?;

    let user_key_opt = user_key_arg
        .map(str::to_owned)
        .or_else(|| env::var("BOSON_USER_PRIVATE_KEY").ok())
        .map(|s| signature::PrivateKey::try_from(s.as_str()))
        .transpose()?
        .map(signature::KeyPair::try_from)
        .transpose()?;

    let Some(user_key) = user_key_opt else {
        return Err(boson::errors::ArgumentError::new(
            "User key must be provided or set in $BOSON_USER_PRIVATE_KEY"
        ));
    };

    let Some(user_id) = user_id_arg else {
        return Err(boson::errors::ArgumentError::new(
            "User ID must be provided or set in $BOSON_USER_ID"
        ));
    };

    if user_id != Id::from(user_key.public_key()) {
        return Err(boson::errors::ArgumentError::new(
            "User ID {user_id}does not match the provided user key"
        ));
    }

    let dir_opts = director::Options::new(&director_url)?
        .with_insecure(cli.insecure)
        .with_user_keypair(user_key);

    let client = Client::new(dir_opts)?;
    let profile = client.get_profile().await?;
    print_user_profile(&profile);

    if devices {
        println!();
        let devices = client.list_devices().await?;
        print_devices(&devices, Some(&user_id));
    }
    Ok(())
}

async fn reguser_command(
    cli: &Cli,
    user_key_arg: &str,
    name_arg: Option<&str>,
    email_arg: Option<&str>,
    bio_arg: Option<&str>,
    passphrase_arg: Option<&str>,
) -> Result<()> {
    let director_url = resolve_director_url(cli, None)?;

    let user_sk = signature::PrivateKey::try_from(user_key_arg)?;
    let user_key = signature::KeyPair::from(&user_sk);
    let user_id = Id::from(user_key.public_key());

    let dir_opts = director::Options::new(&director_url)?
        .with_insecure(cli.insecure)
        .with_user_private_key(user_sk.clone());
    let client = Client::new(dir_opts)?;


    if let Ok(profile) = client.get_profile().await {
        println!("User {user_id} is already registered.");
        println!();
        print_user_profile(&profile);
        if let Ok(devices) = client.list_devices().await {
            println!();
            print_devices(&devices, Some(&user_id));
        }
        return Ok(());
    }

    let mut reg = UserRegistration::new();
    if let Some(name) = name_arg {
        reg = reg.with_name(name);
    }
    if let Some(email) = email_arg {
        reg = reg.with_email(email);
    }
    if let Some(bio) = bio_arg {
        reg = reg.with_bio(bio);
    }

    if let Some(passphrase) = passphrase_arg {
        reg = reg.with_passphrase(passphrase);
    }

    match client.register_user(&reg).await {
        Ok(()) => {
            println!("User {user_id} successfully registered.");
            println!();
            if let Ok(profile) = client.get_profile().await {
                print_user_profile(&profile);
            }
        }
        Err(e) => {
            if let Ok(profile) = client.get_profile().await {
                println!("User {user_id} is already registered.");
                println!();
                print_user_profile(&profile);
                if let Ok(devices) = client.list_devices().await {
                    println!();
                    print_devices(&devices, Some(&user_id));
                }
            } else {
                return Err(e);
            }
        }
    }

    Ok(())
}

async fn regdev_command(
    cli: &Cli,
    device_key_arg: &str,
    user_key_arg: Option<&str>,
    device_name_arg: Option<&str>,
    app_arg: Option<&str>,
    passphrase_arg: Option<&str>,
) -> Result<()> {
    let director_url = resolve_director_url(cli, None)?;

    // Resolve user private key (required to authorize a device)
    let user_sk_str = user_key_arg
        .map(str::to_owned)
        .or_else(|| env::var("BOSON_USER_PRIVATE_KEY").ok())
        .ok_or_else(|| {
            boson::errors::ArgumentError::new(
                "--user-key or $BOSON_USER_PRIVATE_KEY must be specified to authorize device registration",
            )
        })?;
    let user_sk = signature::PrivateKey::try_from(user_sk_str.as_str())?;

    let dev_sk = signature::PrivateKey::try_from(device_key_arg)?;
    let dev_key = signature::KeyPair::from(&dev_sk);
    let device_id = Id::from(dev_key.public_key());

    let dir_opts = director::Options::new(&director_url)?
        .with_insecure(cli.insecure)
        .with_user_private_key(user_sk.clone())
        .with_device_private_key(dev_sk.clone());
    let client = Client::new(dir_opts)?;
    let user_id = client
        .user_id()
        .copied()
        .unwrap_or_else(|| Id::from(signature::KeyPair::from(&user_sk).public_key()));

    // Check if device is already registered
    if let Ok(devices) = client.list_devices().await {
        if let Some(existing) = devices.iter().find(|d| d.id() == &device_id) {
            println!("Device {device_id} is already registered under user {user_id}.");
            println!();
            print_single_device(existing);
            return Ok(());
        }
    }

    let rand_suffix = hex::encode(&Id::random().as_bytes()[..4]);
    let dev_name = device_name_arg
        .map(str::to_string)
        .unwrap_or_else(|| format!("device_{rand_suffix}"));
    let app_name = app_arg.unwrap_or("BosonApp");

    client
        .register_device_with_key(&dev_key, &dev_name, app_name, passphrase_arg)
        .await?;

    println!("Device {device_id} successfully registered under user {user_id}.");
    println!("  Device Private Key : {} (hexstr)", dev_sk.to_hexstr());
    println!("  Device Private Key : {} (base58)", dev_sk.to_base58());
    println!();

    if let Ok(devices) = client.list_devices().await {
        if let Some(d) = devices.iter().find(|d| d.id() == &device_id) {
            print_single_device(d);
        } else {
            print_devices(&devices, Some(&user_id));
        }
    }

    Ok(())
}

fn print_user_profile(profile: &Profile) {
    println!("+------------------------------------------------------------+");
    println!("|                   User Profile Information                 |");
    println!("+------------------------------------------------------------+");
    print_field("User ID", profile.id());
    print_field("Name", profile.name().map(String::as_str).unwrap_or("-"));
    print_field("Email", profile.email().map(String::as_str).unwrap_or("-"));
    print_field("Bio", profile.bio().map(String::as_str).unwrap_or("-"));
    print_field("Plan", profile.plan_name());
    print_field("Admin", if profile.admin() { "yes" } else { "no" });
    print_field(
        "Passphrase",
        if profile.is_passphrase_protected() {
            "protected"
        } else {
            "none"
        },
    );
    print_field("Created At", format_system_time(profile.created_at()));
    print_field("Updated At", format_system_time(profile.updated_at()));
}

fn print_single_device(device: &Device) {
    println!("+------------------------------------------------------------+");
    println!("|                     Device Information                     |");
    println!("+------------------------------------------------------------+");
    print_field("Device ID", device.id());
    print_field("User ID", device.user_id());
    print_field("Name", device.name().unwrap_or("-"));
    print_field("App", device.app().unwrap_or("-"));
    print_field(
        "Last Seen",
        if device.last_seen() > 0 {
            format_timestamp(device.last_seen())
        } else {
            "never".to_string()
        },
    );
    print_field("Last Address", device.last_address().unwrap_or("-"));
    if device.created_at() > 0 {
        print_field("Created At", format_timestamp(device.created_at()));
    }
    if device.updated_at() > 0 {
        print_field("Updated At", format_timestamp(device.updated_at()));
    }
}

fn print_devices(devices: &[Device], user_id: Option<&Id>) {
    println!("+------------------------------------------------------------+");
    println!("|                     Authorized Devices                     |");
    println!("+------------------------------------------------------------+");
    if let Some(uid) = user_id {
        print_field("User ID", uid);
        println!();
    }

    if devices.is_empty() {
        println!("No devices authorized for this user.");
        return;
    }

    let id_width = devices
        .iter()
        .map(|d| d.id().to_string().len())
        .max()
        .unwrap_or("Device ID".len())
        .max("Device ID".len());

    println!(
        "{:<4} {:<id_width$} {:<16} {:<16} {}",
        "#", "Device ID", "Name", "App", "Last Seen"
    );
    println!("{}", "-".repeat(id_width + 44));

    for (index, device) in devices.iter().enumerate() {
        let id_str = device.id().to_string();
        let name = device.name().unwrap_or("-");
        let app = device.app().unwrap_or("-");
        let last_seen = if device.last_seen() > 0 {
            format_timestamp(device.last_seen())
        } else {
            "never".to_string()
        };
        println!(
            "{:<4} {:<id_width$} {:<16} {:<16} {}",
            index + 1,
            id_str,
            name,
            app,
            last_seen
        );
    }
}

fn format_timestamp(timestamp_ms: u64) -> String {
    let secs = timestamp_ms / 1000;
    format!("{secs} seconds since Unix epoch")
}

fn format_system_time(time: std::time::SystemTime) -> String {
    match time.duration_since(UNIX_EPOCH) {
        Ok(duration) => format!("{} seconds since Unix epoch", duration.as_secs()),
        Err(_) => "before Unix epoch".to_string(),
    }
}

fn print_status(endpoint: &str, status: &NodeStatus, authenticated_user: Option<&Id>) {
    println!("+------------------------------------------------------------+");
    println!("|                  Boson Super Node Information              |");
    println!("+------------------------------------------------------------+");
    print_field("Node ID", status.node_id());
    print_field("Endpoint", endpoint);
    print_field("Software", status.software().unwrap_or("-"));
    print_field("Version", status.version().unwrap_or("-"));
    print_field("Name", status.name().unwrap_or("-"));
    print_field("Website", status.website().unwrap_or("-"));
    print_field("Contact", status.contact().unwrap_or("-"));
    print_field("Logo", status.logo().unwrap_or("-"));
    print_field("Running", if status.is_running() { "yes" } else { "no" });
    print_field("Started At", format_started_at(status));
    print_field(
        "Authenticated User",
        authenticated_user
            .map(ToString::to_string)
            .unwrap_or_else(|| "-".to_string()),
    );

    println!();
    print_services(status.services());
}

fn print_field(label: &str, value: impl std::fmt::Display) {
    println!("{label:>20}: {value}");
}

fn format_started_at(status: &NodeStatus) -> String {
    match status.started_at().duration_since(UNIX_EPOCH) {
        Ok(duration) => format!("{} seconds since Unix epoch", duration.as_secs()),
        Err(_) => "before Unix epoch".to_string(),
    }
}

fn print_services(services: &[Service]) {
    println!("+------------------------------------------------------------+");
    println!("|                    Advertised Services                     |");
    println!("+------------------------------------------------------------+");

    if services.is_empty() {
        println!("No services advertised by this Director node.");
        return;
    }

    let peer_id_width = services
        .iter()
        .map(|service| service.peer_id().to_string().len())
        .max()
        .unwrap_or("Peer ID".len())
        .max("Peer ID".len());

    for (index, service) in services.iter().enumerate() {
        let peer_id = service.peer_id().to_string();
        println!(
            "{:<4} {:<16} {:<28} {:<peer_id_width$} {}",
            index + 1,
            service.service_id(),
            service.service_name().unwrap_or("-"),
            peer_id,
            service.endpoint().unwrap_or("-")
        );
    }
}

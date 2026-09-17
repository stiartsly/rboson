use boson::{
    director::{Client, NotFoundError, Options, Profile, UnauthorizedError, UserRegistration},
    signature::PrivateKey,
    Result,
};
use serde::Deserialize;
use std::{fs, path::Path};

pub(crate) struct Session {
    client: Client,
    logged_in: bool,
}

impl Session {
    pub(crate) fn load_from(path: impl AsRef<Path>) -> Result<Self> {
        let options = load_options(path)?;
        Ok(Self {
            client: Client::new(options)?,
            logged_in: false,
        })
    }

    pub(crate) async fn login(&mut self) {
        let profile = match self.client.get_profile().await {
            Ok(profile) => profile,
            Err(e)
                if e.downcast_ref::<UnauthorizedError>().is_some()
                    || e.downcast_ref::<NotFoundError>().is_some() =>
            {
                if let Err(e) = self.client.register_user().await {
                    println!("\x1b[31mUnable to register the director account: {e}\x1b[0m");
                    return;
                }
                match self.client.get_profile().await {
                    Ok(profile) => profile,
                    Err(e) => {
                        println!(
                            "\x1b[31mRegistered the director account, but could not fetch its profile: {e}\x1b[0m"
                        );
                        return;
                    }
                }
            }
            Err(e) => {
                println!("\x1b[31mUnable to log in to the director: {e}\x1b[0m");
                return;
            }
        };

        println!("\x1b[32mLogged in to the director successfully.\x1b[0m");
        print_profile(&profile);
        self.logged_in = true;
    }

    pub(crate) async fn show_me(&self) {
        match self.client.fetch_node_id().await {
            Ok(node_id) => println!("Super node ID: {node_id}"),
            Err(e) => println!("\x1b[31mUnable to fetch the super node ID: {e}\x1b[0m"),
        }

        if self.logged_in {
            match self.client.get_profile().await {
                Ok(profile) => print_profile(&profile),
                Err(e) => {
                    println!("\x1b[31mUnable to fetch the current user profile: {e}\x1b[0m")
                }
            }
        }
    }
}

fn print_profile(profile: &Profile) {
    println!("Current user profile:");
    println!("  User ID              : {}", profile.id());
    println!(
        "  Name                 : {}",
        profile.name().map_or("-", String::as_str)
    );
    println!(
        "  Email                : {}",
        profile.email().map_or("-", String::as_str)
    );
    println!(
        "  Bio                  : {}",
        profile.bio().map_or("-", String::as_str)
    );
    println!(
        "  Avatar               : {}",
        profile.avatar().map_or("-", String::as_str)
    );
    println!("  Administrator        : {}", profile.admin());
    println!("  Plan                 : {}", profile.plan_name());
    println!(
        "  Passphrase protected : {}",
        profile.is_passphrase_protected()
    );
    println!("  Created              : {:?}", profile.created_at());
    println!("  Updated              : {:?}", profile.updated_at());
}

#[derive(Deserialize)]
struct DirectorConfig {
    director: DirectorSection,
    user: UserSection,
    device: DeviceSection,
}

#[derive(Deserialize)]
struct DirectorSection {
    url: String,
    #[serde(rename = "nodeId")]
    node_id: String,
    insecure: bool,
}

#[derive(Deserialize)]
struct UserSection {
    id: String,
    #[serde(rename = "privateKey")]
    private_key: String,
    name: String,
    email: String,
    bio: String,
    passphrase: String,
}

#[derive(Deserialize)]
struct DeviceSection {
    #[serde(rename = "privateKey")]
    private_key: String,
    name: String,
    app: String,
}

fn load_options(path: impl AsRef<Path>) -> Result<Options> {
    let content = fs::read_to_string(path)?;
    let config: DirectorConfig = serde_yaml::from_str(&content)?;
    let user_id = config.user.id.parse()?;
    let user_private_key = PrivateKey::try_from(config.user.private_key.as_str())?;
    let device_private_key = PrivateKey::try_from(config.device.private_key.as_str())?;
    let registration = UserRegistration::new()
        .with_name(config.user.name)
        .with_email(config.user.email)
        .with_bio(config.user.bio)
        .with_passphrase(config.user.passphrase)
        .with_initial_device(config.device.name, config.device.app);

    Ok(Options::new(config.director.url)?
        .with_node_id(config.director.node_id.parse()?)
        .with_user_id(user_id)
        .with_user_private_key(user_private_key)
        .with_device_private_key(device_private_key)
        .with_registration(registration)
        .with_insecure(config.director.insecure))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_loads_director_config() {
        let session = Session::load_from("apps/shell/director.yaml").unwrap();

        assert_eq!(session.client.options().director_url().scheme(), "https");
        assert!(session.client.options().is_insecure());
        assert!(session.client.options().user_private_key().is_some());
        assert!(session.client.options().device_private_key().is_some());
        assert!(!session.logged_in);
    }

    #[test]
    fn session_reports_missing_director_config() {
        assert!(Session::load_from("apps/shell/missing-director.yaml").is_err());
    }
}

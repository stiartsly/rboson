use boson::{
    director::{Client, NotFoundError, Profile, UnauthorizedError},
    Result,
};
use clap::Command;

use crate::config::ShellConfig;

pub(crate) struct Session {
    client: Client,
    logged_in: bool,
}

impl Session {
    pub(crate) fn new(config: &ShellConfig) -> Result<Self> {
        Ok(Self {
            client: Client::new(config.director_options()?)?,
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

pub(crate) fn command() -> Command {
    Command::new("login")
        .about("Log in to the super node, registering the configured account if necessary")
}

pub(crate) async fn run(session: &mut Session) {
    session.login().await;
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

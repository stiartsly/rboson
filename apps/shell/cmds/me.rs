use clap::Command;

use super::login::Session;

pub(crate) fn command() -> Command {
    Command::new("me").about("Show the super node and current user profile")
}

pub(crate) async fn run(session: &Session) {
    session.show_me().await;
}

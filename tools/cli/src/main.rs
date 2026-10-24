/*
 * Copyright (c) 2023 -      bosonnetwork.io
 *
 * Permission is hereby granted, free of charge, to any person obtaining a copy
 * of this software and associated documentation files (the "Software"), to deal
 * in the Software without restriction, including without limitation the rights
 * to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
 * copies of the Software, and to permit persons to whom the Software is
 * furnished to do so, subject to the following conditions:
 *
 * The above copyright notice and this permission notice shall be included in all
 * copies or substantial portions of the Software.
 *
 * THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
 * IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
 * FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
 * AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
 * LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
 * OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
 * SOFTWARE.
 */

//! `boson-cli`: the Boson command line client.
//!
//! The command tree is the same as the Java `boson-cli`. Commands without a Rust implementation
//! yet are accepted and report that they are not implemented.

mod director;
mod keygen;
mod proxy;

use clap::{Args, Parser, Subcommand};

use director::ConnectionOptions;

/// The Boson command line client.
#[derive(Parser, Debug)]
#[command(name = "boson-cli", version)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Register with the super node, and manage your account.
    User(UserGroup),
    /// List, register and remove the devices of your account.
    Device(DeviceGroup),
    /// Store, retrieve, list and remove objects in the super node's Ion Store.
    #[command(subcommand)]
    Ionstore(IonStoreCommand),
    /// Use the Boson DHT through the super node's web gateway.
    #[command(subcommand)]
    Dht(DhtCommand),
    /// Expose a service on this machine through the super node's Active Proxy.
    #[command(subcommand)]
    Proxy(ProxyCommand),
    /// Get credentials for the super node's TURN relay.
    #[command(subcommand)]
    Turn(TurnCommand),
    /// Show the super node's id and status. These need no identity.
    Node(NodeGroup),
    /// Show and change the configuration.
    #[command(subcommand)]
    Config(ConfigCommand),
    /// Create, import or show the identity this tool acts with.
    #[command(subcommand)]
    Identity(IdentityCommand),
    /// Work with Boson keys and encodings, offline.
    #[command(subcommand)]
    Util(UtilCommand),
}

/// The arguments of a command that is not implemented yet: accepted, and ignored.
#[derive(Args, Debug)]
struct Pending {
    #[arg(trailing_var_arg = true, allow_hyphen_values = true, hide = true)]
    _args: Vec<String>,
}

#[derive(Args, Debug)]
struct UserGroup {
    #[command(flatten)]
    connection: ConnectionOptions,
    #[command(subcommand)]
    command: UserCommand,
}

#[derive(Subcommand, Debug)]
enum UserCommand {
    /// Register your user with the super node.
    Register(director::UserRegisterArgs),
    /// Show your profile.
    Show(director::UserShowArgs),
    /// Change your profile.
    Update(Pending),
    /// Save or change your avatar.
    #[command(subcommand)]
    Avatar(AvatarCommand),
    /// Set, change or remove the account passphrase.
    #[command(subcommand)]
    Passphrase(PassphraseCommand),
}

#[derive(Subcommand, Debug)]
enum AvatarCommand {
    /// Save your avatar to a file.
    Get(Pending),
    /// Upload an image as your avatar.
    Set(Pending),
}

#[derive(Subcommand, Debug)]
enum PassphraseCommand {
    /// Set a passphrase on an account that has none.
    Set(Pending),
    /// Change the account passphrase.
    Change(Pending),
    /// Remove the passphrase from the account.
    Clear(Pending),
}

#[derive(Args, Debug)]
struct DeviceGroup {
    #[command(flatten)]
    connection: ConnectionOptions,
    #[command(subcommand)]
    command: DeviceCommand,
}

#[derive(Subcommand, Debug)]
enum DeviceCommand {
    /// List the devices registered to your account.
    List(Pending),
    /// Register a device to your account.
    Add(director::DeviceAddArgs),
    /// Remove a device from your account. It can no longer act for you.
    Remove(Pending),
}

#[derive(Subcommand, Debug)]
enum IonStoreCommand {
    /// Store a file as an object.
    Put(Pending),
    /// Retrieve an object.
    Get(Pending),
    /// List your objects, newest first.
    List(Pending),
    /// Show an object's details, without retrieving it.
    Show(Pending),
    /// Remove your objects.
    Remove(Pending),
}

#[derive(Subcommand, Debug)]
enum DhtCommand {
    /// Show who this machine is on the DHT, and the gateway it uses.
    Id(Pending),
    /// Look up a node, a value or a peer on the DHT.
    #[command(subcommand)]
    Find(DhtFindCommand),
    /// Store a value on the DHT.
    Store(Pending),
    /// Announce a peer on the DHT.
    Announce(Pending),
    /// Manage the values the gateway keeps announced for you.
    #[command(subcommand)]
    Value(DhtAnnouncedCommand),
    /// Manage the peers the gateway keeps announced for you.
    #[command(subcommand)]
    Peer(DhtAnnouncedCommand),
}

#[derive(Subcommand, Debug)]
enum DhtFindCommand {
    /// Find a node, and show its addresses.
    Node(Pending),
    /// Find a value, and show it.
    Value(Pending),
    /// Find the peers announced for an id.
    Peer(Pending),
}

#[derive(Subcommand, Debug)]
enum DhtAnnouncedCommand {
    /// List what the gateway keeps announced for you.
    List(Pending),
    /// Show one of them.
    Show(Pending),
    /// Stop the gateway keeping it announced.
    Remove(Pending),
}

#[derive(Subcommand, Debug)]
enum ProxyCommand {
    /// Start the proxy, and keep it running until Ctrl+C.
    Start(proxy::Options),
}

#[derive(Subcommand, Debug)]
enum TurnCommand {
    /// Get time-limited TURN credentials.
    Credentials(Pending),
}

#[derive(Args, Debug)]
struct NodeGroup {
    #[command(flatten)]
    connection: ConnectionOptions,
    #[command(subcommand)]
    command: NodeCommand,
}

#[derive(Subcommand, Debug)]
enum NodeCommand {
    /// Show the super node's id, as its Director reports it.
    Id(Pending),
    /// Show what the super node runs and which services it offers.
    Status,
}

#[derive(Subcommand, Debug)]
enum ConfigCommand {
    /// Create the configuration file.
    Init(Pending),
    /// Show the settings in effect, and where each one comes from.
    Show(Pending),
    /// Change a setting in the configuration file.
    Set(Pending),
    /// Remove a setting from the configuration file.
    Unset(Pending),
}

#[derive(Subcommand, Debug)]
enum IdentityCommand {
    /// Create a new identity file with a new key.
    Create(Pending),
    /// Create the identity file from an existing private key.
    Import(Pending),
    /// Show the id of the identity in effect, and where it comes from.
    Show(Pending),
}

#[derive(Subcommand, Debug)]
enum UtilCommand {
    /// Generate a new key pair.
    Keygen(keygen::Options),
    /// Check that a key is valid.
    CheckKey(Pending),
    /// Print the public key of a private key.
    PublicKey(Pending),
    /// Sign a binding nonce with your identity.
    Sign(Pending),
    /// Convert a hex value to Base58.
    HexToBase58(Pending),
    /// Convert a Base58 value to hex, with 0x.
    Base58ToHex(Pending),
}

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let cli = Cli::parse();
    let result = match cli.command {
        Command::User(group) => match group.command {
            UserCommand::Register(args) => director::user_register(&group.connection, &args).await,
            UserCommand::Show(args) => director::user_show(&group.connection, &args).await,
            UserCommand::Update(_) => pending("user update"),
            UserCommand::Avatar(AvatarCommand::Get(_)) => pending("user avatar get"),
            UserCommand::Avatar(AvatarCommand::Set(_)) => pending("user avatar set"),
            UserCommand::Passphrase(PassphraseCommand::Set(_)) => pending("user passphrase set"),
            UserCommand::Passphrase(PassphraseCommand::Change(_)) => {
                pending("user passphrase change")
            }
            UserCommand::Passphrase(PassphraseCommand::Clear(_)) => {
                pending("user passphrase clear")
            }
        },
        Command::Device(group) => match group.command {
            DeviceCommand::List(_) => pending("device list"),
            DeviceCommand::Add(args) => director::device_add(&group.connection, &args).await,
            DeviceCommand::Remove(_) => pending("device remove"),
        },
        Command::Ionstore(command) => pending(match command {
            IonStoreCommand::Put(_) => "ionstore put",
            IonStoreCommand::Get(_) => "ionstore get",
            IonStoreCommand::List(_) => "ionstore list",
            IonStoreCommand::Show(_) => "ionstore show",
            IonStoreCommand::Remove(_) => "ionstore remove",
        }),
        Command::Dht(command) => pending(match command {
            DhtCommand::Id(_) => "dht id",
            DhtCommand::Find(DhtFindCommand::Node(_)) => "dht find node",
            DhtCommand::Find(DhtFindCommand::Value(_)) => "dht find value",
            DhtCommand::Find(DhtFindCommand::Peer(_)) => "dht find peer",
            DhtCommand::Store(_) => "dht store",
            DhtCommand::Announce(_) => "dht announce",
            DhtCommand::Value(_) => "dht value",
            DhtCommand::Peer(_) => "dht peer",
        }),
        Command::Proxy(ProxyCommand::Start(options)) => {
            proxy::start(options).await;
            Ok(())
        }
        Command::Turn(TurnCommand::Credentials(_)) => pending("turn credentials"),
        Command::Node(group) => match group.command {
            NodeCommand::Id(_) => pending("node id"),
            NodeCommand::Status => director::node_status(&group.connection).await,
        },
        Command::Config(command) => pending(match command {
            ConfigCommand::Init(_) => "config init",
            ConfigCommand::Show(_) => "config show",
            ConfigCommand::Set(_) => "config set",
            ConfigCommand::Unset(_) => "config unset",
        }),
        Command::Identity(command) => pending(match command {
            IdentityCommand::Create(_) => "identity create",
            IdentityCommand::Import(_) => "identity import",
            IdentityCommand::Show(_) => "identity show",
        }),
        Command::Util(command) => match command {
            UtilCommand::Keygen(options) => {
                keygen::run(options);
                Ok(())
            }
            UtilCommand::CheckKey(_) => pending("util check-key"),
            UtilCommand::PublicKey(_) => pending("util public-key"),
            UtilCommand::Sign(_) => pending("util sign"),
            UtilCommand::HexToBase58(_) => pending("util hex-to-base58"),
            UtilCommand::Base58ToHex(_) => pending("util base58-to-hex"),
        },
    };

    if let Err(error) = result {
        eprintln!("boson-cli: {error}");
        std::process::exit(1);
    }
}

/// Reports a command of the Java `boson-cli` that has no Rust implementation yet.
fn pending(command: &str) -> boson::Result<()> {
    eprintln!("boson-cli {command}: not implemented yet");
    std::process::exit(2);
}

#[cfg(test)]
mod tests {
    use clap::CommandFactory;

    #[test]
    fn command_tree_is_valid() {
        super::Cli::command().debug_assert();
    }
}

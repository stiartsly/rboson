use boson::{
    signature::{KeyPair, PrivateKey},
    Id,
};
use clap::{arg, ArgMatches, Command};

pub(crate) fn command() -> Command {
    Command::new("identity")
        .about("Show this shell user identity, or generate a new random identity")
        .arg(arg!(-g --generate "Generate a new random identity"))
}

pub(crate) fn run(matches: &ArgMatches, configured_user_key: &PrivateKey) {
    let keypair = if matches.get_flag("generate") {
        KeyPair::random()
    } else {
        KeyPair::from(configured_user_key)
    };
    print_identity(&keypair);
    println!("\nKeep the private key secret. It controls this identity.");
}

fn print_identity(keypair: &KeyPair) {
    let id = Id::from(keypair.public_key());
    println!("  User ID     : {}", id.to_base58());
    println!("  DID         : {}", id.to_did_string());
    println!("  Public Key  : {}", keypair.public_key());
    println!(
        "  Private Key : {} (base58)",
        keypair.private_key().to_base58()
    );
    println!(
        "              : {} (hex)",
        keypair.private_key().to_hexstr()
    );
}

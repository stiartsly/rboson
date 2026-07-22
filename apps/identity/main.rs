use clap::Parser;

use boson::CryptoIdentity;

#[derive(Parser, Debug)]
#[command(name = "identity")]
#[command(about = "Generate a Boson user identity", long_about = None)]
struct Options {}

fn main() {
    Options::parse();

    let identity = CryptoIdentity::new();
    let keypair = identity.keypair();
    let id = identity.id();

    println!("+--------------------------------------------------------------+");
    println!("|                 Boson User Identity Created                 |");
    println!("+--------------------------------------------------------------+");
    println!("  User ID     : {}", id.to_base58());
    println!("  DID         : {}", id.to_did_string());
    println!("  Public Key  : {}", keypair.public_key());
    println!("  Private Key : {}", keypair.private_key().to_hexstr());
    println!();
    println!("Keep the private key secret. It controls this identity.");
}

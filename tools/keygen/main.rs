use clap::Parser;

use boson::signature::KeyPair;
use boson::{CryptoIdentity, Id, Identity};

#[derive(Parser, Debug)]
#[command(name = "keygen")]
#[command(about = "Generate a Boson user identity or Device key", long_about = None)]
struct Options {
    /// Generate a Boson user identity (default)
    #[arg(short = 'u', long = "user")]
    user: bool,

    /// Generate a Boson device key
    #[arg(short = 'd', long = "device")]
    device: bool,
}

fn main() {
    let options = Options::parse();
    let generate_device = options.device;
    let generate_user = options.user || !options.device;

    if generate_user {
        generate_user_identity();
    }

    if generate_device {
        if generate_user {
            println!();
        }
        generate_device_key();
    }
}

fn generate_user_identity() {
    let identity = CryptoIdentity::new();
    let keypair = identity.signature_keypair();
    let id = identity.id();

    println!("+--------------------------------------------------------------+");
    println!("|                 Boson User Identity Created                  |");
    println!("+--------------------------------------------------------------+");
    println!("  User ID     : {} (base58)", id.to_base58());
    println!("  DID         : {}", id.to_did_string());
    println!("  Private Key : {} (hexstr)", keypair.private_key().to_hexstr());
    println!("  Private Key : {} (base58)", keypair.private_key().to_base58());
    println!();
    println!("Keep the private key secret. It controls this identity.");
}

fn generate_device_key() {
    let keypair = KeyPair::random();
    let device_id = Id::from(keypair.public_key());

    println!("+--------------------------------------------------------------+");
    println!("|                  Boson Device Key Created                    |");
    println!("+--------------------------------------------------------------+");
    println!("  Device ID   : {} (base58)", device_id.to_base58());
    println!("  Private Key : {} (hexstr)", keypair.private_key().to_hexstr());
    println!("  Private Key : {} (base58)", keypair.private_key().to_base58());
    println!();
    println!("Keep the private key secret. It controls this device identity.");
}

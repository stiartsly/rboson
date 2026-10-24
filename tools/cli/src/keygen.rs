// Copyright (c) 2023 -      bosonnetwork.io
//
// Permission is hereby granted, free of charge, to any person obtaining a copy
// of this software and associated documentation files (the "Software"), to deal
// in the Software without restriction, including without limitation the rights
// to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
// copies of the Software, and to permit persons to whom the Software is
// furnished to do so, subject to the following conditions:
//
// The above copyright notice and this permission notice shall be included in all
// copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
// IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
// FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
// AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
// LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
// OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
// SOFTWARE.

use clap::Args;

use boson::signature::KeyPair;
use boson::{CryptoIdentity, Id, Identity};

#[derive(Args, Debug)]
pub(crate) struct Options {
    /// Generate a Boson user identity (default)
    #[arg(short = 'u', long = "user")]
    user: bool,

    /// Generate a Boson device key
    #[arg(short = 'd', long = "device")]
    device: bool,
}

pub(crate) fn run(options: Options) {
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
    let keypair = identity.signature_key();
    let id = identity.id();

    println!("+--------------------------------------------------------------+");
    println!("|                 Boson User Identity Created                  |");
    println!("+--------------------------------------------------------------+");
    println!("  User ID     : {} (base58)", id.to_base58());
    println!("  DID         : {}", id.to_did_string());
    println!(
        "  Private Key : {} (hexstr)",
        keypair.private_key().to_hexstr()
    );
    println!(
        "  Private Key : {} (base58)",
        keypair.private_key().to_base58()
    );
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
    println!(
        "  Private Key : {} (hexstr)",
        keypair.private_key().to_hexstr()
    );
    println!(
        "  Private Key : {} (base58)",
        keypair.private_key().to_base58()
    );
    println!();
    println!("Keep the private key secret. It controls this device identity.");
}

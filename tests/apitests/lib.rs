#[cfg(test)]
mod core {
    mod id;
    mod signature;
    mod cryptobox;
    mod node_info;
    mod peer_info;
    mod value;
   // mod config;
}

#[cfg(test)]
mod dht {
    mod node;
}

#[cfg(test)]
mod did {
    mod didurl;
    mod verification_method;
    mod credential;
    mod vouch;
    mod card;
    mod vc;
    mod vp;
    mod diddoc;
}

/*
#[cfg(test)]
mod messaging {
    mod client;
}
*/

// helper function
fn randomize_bytes<const N: usize>(array: &mut [u8; N]) {
    unsafe {
        libsodium_sys::randombytes_buf(
            array.as_mut_ptr() as *mut libc::c_void,
            N
        );
    }
}

fn create_random_bytes(len: usize) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(len);
    unsafe {
        libsodium_sys::randombytes_buf(
            bytes.as_mut_ptr()as *mut libc::c_void,
            len
        );
        bytes.set_len(len);
    }
    bytes
}

fn remove_working_path(input: &str) {
    if std::fs::metadata(&input).is_ok() {
        match std::fs::remove_dir_all(&input) {
            Ok(_) => {}
            Err(e) => {
                panic!("Failed to remove directory: {}", e);
            }
        }
    }
}

fn main() {}

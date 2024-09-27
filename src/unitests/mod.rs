#[cfg(test)] mod test_id;
#[cfg(test)] mod test_peer_info;
#[cfg(test)] mod test_node_info;
#[cfg(test)] mod test_value;
#[cfg(test)] mod test_sqlite_storage;
#[cfg(test)] mod test_token_man;
#[cfg(test)] mod test_routing_table;
#[cfg(test)] mod test_node_runner;
#[cfg(test)] mod test_version;

#[cfg(test)] use std::env;
#[cfg(test)] use std::fs;
#[cfg(test)] use libsodium_sys::randombytes_buf;

#[cfg(test)]
fn create_random_bytes(len: usize) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(len);
    unsafe {
        randombytes_buf(
            bytes.as_mut_ptr() as *mut libc::c_void,
            len
        );
        bytes.set_len(len);
    };
    bytes
}

#[cfg(test)]
fn working_path(input: &str) -> String {
    let path = env::current_dir().unwrap().join(input);
    if !fs::metadata(&path).is_ok() {
        match fs::create_dir(&path) {
            Ok(_) => {}
            Err(e) => {
                panic!("Failed to create directory: {}", e);
            }
        }
    }
    path.display().to_string()
}

#[cfg(test)]
fn remove_working_path(input: &str) {
    if fs::metadata(&input).is_ok() {
        match fs::remove_dir_all(&input) {
            Ok(_) => {}
            Err(e) => {
                panic!("Failed to remove directory: {}", e);
            }
        }
    }
}
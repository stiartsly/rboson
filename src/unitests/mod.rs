#[cfg(test)] mod test_id;
#[cfg(test)] mod test_peer_info;
#[cfg(test)] mod test_node_info;
#[cfg(test)] mod test_value;
#[cfg(test)] mod test_sqlite_storage;
#[cfg(test)] mod test_token_man;
#[cfg(test)] mod test_routing_table;
#[cfg(test)] mod test_node_runner;
#[cfg(test)] mod test_version;
#[cfg(test)] mod test_addr;
#[cfg(test)] mod test_logger;

#[cfg(test)] mod test_find_node_req;
#[cfg(test)] mod test_find_node_rsp;
#[cfg(test)] mod test_find_peer_req;
#[cfg(test)] mod test_find_peer_rsp;

#[cfg(test)] use std::env;
#[cfg(test)] use std::fs;

#[cfg(test)]
#[allow(non_upper_case_globals)]
static create_random_bytes: fn(usize) -> Vec<u8> = crate::random_bytes;

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

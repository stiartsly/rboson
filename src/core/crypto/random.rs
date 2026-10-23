use libsodium_sys::randombytes_buf;

pub fn random_array_secure<const N: usize>(array: &mut [u8; N]) {
    unsafe {
        randombytes_buf(
            array.as_mut_ptr() as *mut libc::c_void,
            N
        );
    }
}

pub fn secure_array<const N: usize>() -> [u8; N] {
    let mut array = [0u8; N];
    random_array_secure(&mut array);
    array
}

pub fn random_bytes_secure(buffer: &mut [u8]) {
    if buffer.is_empty() {
        return;
    }
    unsafe {
        randombytes_buf(
            buffer.as_mut_ptr() as *mut libc::c_void,
            buffer.len(),
        );
    }
}

pub fn secure_bytes(length: usize) -> Vec<u8> {
    let mut bytes = vec![0u8; length];
    random_bytes_secure(&mut bytes);
    bytes
}

pub fn bytes(length: usize) -> Vec<u8> {
    let mut bytes = vec![0u8; length];
    rand::fill(bytes.as_mut_slice());
    bytes
}

pub fn fill(buffer: &mut [u8]) {
    rand::fill(buffer);
}

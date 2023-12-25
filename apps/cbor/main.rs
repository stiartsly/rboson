use std::net::UdpSocket;

fn main() -> std::io::Result<()> {
    let socket = UdpSocket::bind("192.168.1.105:30000")?;
    let message = "Hello, server!";
    let result = socket.send_to(message.as_bytes(), "192.168.1.105:32222");
    match result {
        Ok(size) => {
            println!(" size: {}", size);
        },
        Err(err) => {
            println!("error {}", err);
        }
    }
    println!("msg has sent ");

    Ok(())
}

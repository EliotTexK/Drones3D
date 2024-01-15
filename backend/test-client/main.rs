use std::net::TcpStream;
use std::io::{Read, Write};

fn main() {
    let mut stream = TcpStream::connect("127.0.0.1:44556").expect("Could not connect to the server");
    stream.write(b"COMPETITOR\n").expect("Failed to write to server");

    let mut buffer = [0; 1024];
    loop {
        let bytes_read = stream.read(&mut buffer).expect("Failed to read from server");
        if bytes_read == 0 { return; } // Connection closed

        let msg = String::from_utf8_lossy(&buffer[..bytes_read]);
        println!("Client got message: {}", msg);

        let response = b"{rot_y:0,forward_back:1,up_down:0,shoot:false}{rot_y:0,forward_back:1,up_down:0,shoot:false}\n";
        stream.write(response).expect("Failed to write to server");
    }
}
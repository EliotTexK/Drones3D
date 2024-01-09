use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::thread;

fn handle_client(
    mut stream: TcpStream,
    counter: Arc<Mutex<i32>>,
    clients: Arc<Mutex<HashMap<String, TcpStream>>>,
    bidirectional_clients: Arc<Mutex<Vec<String>>>,
) {
    let mut assembled_msg = Vec::new();
    let mut identifier = String::new();

    loop {
        let mut buf = [0; 512];
        let bytes_read = stream
            .read(&mut buf)
            .expect("Could not read from the stream");
        if bytes_read == 0 {
            return;
        } // Connection closed

        assembled_msg.extend_from_slice(&buf[..bytes_read]);

        println!("Parsing buffer for newline delimiter");
        if let Some(newline_index) = assembled_msg.iter().position(|&x| x == b'\n') {
            let msg = String::from_utf8_lossy(&assembled_msg[..newline_index]).to_string();
            assembled_msg.drain(..newline_index + 1);

            if identifier.is_empty() {
                identifier = msg.trim().to_string() + clients.lock().unwrap().len().to_string().as_str();
                clients
                    .lock()
                    .unwrap()
                    .insert(identifier.clone(), stream.try_clone().unwrap());
                println!("New client identifier {}", identifier);
            }

            match msg.trim() {
                "bidirectional" => {
                    let mut bidirectional_clients = bidirectional_clients.lock().unwrap();
                    if bidirectional_clients.len() < 2 {
                        bidirectional_clients.push(identifier.clone());
                    }
                    let count = counter.lock().unwrap();
                    stream
                        .write(format!("{}", *count).as_bytes())
                        .expect("Could not write to the stream");
                    println!("Server sent count: {}", count);
                }
                "unidirectional" => {
                    println!("Got unidirectional client identifier");
                    let count = counter.lock().unwrap();
                    stream
                        .write(format!("{}", *count).as_bytes())
                        .expect("Could not write to the stream");
                    println!("Server sent count: {}", count);
                }
                "+" | "-" => {
                    let mut count = counter.lock().unwrap();
                    *count += if msg.trim() == "+" { 1 } else { -1 };
                    stream
                        .write(format!("{}", *count).as_bytes())
                        .expect("Could not write to the stream");
                    println!("Server sent count to client {}: {}", identifier, count);
                }
                _ => (),
            }
        } else {
            println!("Buffer did not have newline. Reading some more");
        }
    }
}

fn main() {
    let listener = TcpListener::bind("127.0.0.1:44556").unwrap();
    let counter = Arc::new(Mutex::new(0));
    let clients = Arc::new(Mutex::new(HashMap::new()));
    let bidirectional_clients = Arc::new(Mutex::new(Vec::new()));

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                println!("Connected to client!");
                let counter = Arc::clone(&counter);
                let clients = Arc::clone(&clients);
                let bidirectional_clients = Arc::clone(&bidirectional_clients);
                thread::spawn(move || {
                    handle_client(stream, counter, clients, bidirectional_clients)
                });
            }
            Err(e) => {
                eprintln!("Unable to connect: {}", e);
            }
        }
    }
}

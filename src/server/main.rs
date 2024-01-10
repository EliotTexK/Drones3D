use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex, Barrier};
use std::thread;

// TCP buffer size for incoming client messages
const BUFFER_SIZE: usize = 512;

enum ConnectionType {
    Unknown,
    CompetitorA,
    CompetitorB,
    Spectator
}

fn read_until_newline(stream: &mut TcpStream) -> String {
    let mut assembled_msg = Vec::new();
    loop {
        // Create buffer for TCP stream
        let mut buf = [0; BUFFER_SIZE];
        // Read bytes into the buffer
        println!("Reading bytes");
        let bytes_read = stream
            .read(&mut buf)
            .expect("Could not read from the stream");
        if bytes_read == 0 {
            println!("Client disconnected");
            return String::from("ERR");
        }
        println!("Done reading bytes");

        // Extend the assembled message with buffer contents
        assembled_msg.extend_from_slice(&buf[..bytes_read]);

        // Attempt to read up to delimiter
        if let Some(newline_index) = assembled_msg.iter().position(|&x| x == b'\n') {
            return String::from_utf8_lossy(&assembled_msg[..newline_index]).to_string();
        } else {
            println!("Buffer did not have newline. Reading some more");
        }
    }
}

fn handle_client(
    mut stream: TcpStream,
    counter: Arc<Mutex<i32>>,
    num_competitors: Arc<Mutex<u32>>,
    num_spectators: Arc<Mutex<u32>>,
    recieved_inputs: Arc<Barrier>,
    computed_gamestate: Arc<Barrier>
) {
    let mut connection_type: ConnectionType = ConnectionType::Unknown;

    loop {
        let msg: String;

        match connection_type {
            ConnectionType::Unknown => {
                msg = read_until_newline(&mut stream);
                match msg.trim() {
                    "COMPETITOR" => {
                        let mut num_competitors = num_competitors.lock().unwrap();
                        // first to connect is CompetitorA, second is CompetitorB
                        if *num_competitors == 0 {
                            *num_competitors += 1;
                            connection_type = ConnectionType::CompetitorA;
                        } else if *num_competitors == 1 {
                            *num_competitors += 1;
                            connection_type = ConnectionType::CompetitorB;
                        } else {
                            println!("Reached max competitors already: disconnect");
                            return;
                        }
                        stream
                            .write(format!("{}\n", *counter.lock().unwrap()).as_bytes())
                            .expect("Could not write to the stream");
                        println!("Broadcasted initial gamestate");
                    },
                    "SPECTATOR" => {
                        let mut num_spectators = num_spectators.lock().unwrap();
                        if *num_spectators > 0 {
                            println!("Already have spectator: disconnect");
                            return;
                        } else {
                            connection_type = ConnectionType::Spectator;
                            *num_spectators = 1;
                        }
                        stream
                            .write(format!("{}\n", *counter.lock().unwrap()).as_bytes())
                            .expect("Could not write to the stream");
                        println!("Broadcasted initial gamestate");
                    },
                    _ => {
                        println!("Client failed to identify itself: disconnect");
                        return;
                    }
                }
            },
            ConnectionType::CompetitorA | ConnectionType::CompetitorB => {
                msg = read_until_newline(&mut stream);
                // wait for the other competitor to send data
                println!("Waiting for both inputs to be recieved");
                recieved_inputs.wait();
                println!("Recieved both inputs");
                match msg.trim() {
                    "+" => {
                        *counter.lock().unwrap() += 1;
                    },
                    "-" => {
                        *counter.lock().unwrap() -= 1;
                    },
                    _ => {
                        println!("Competitor sent invalid input: disconnect");
                        return;
                    }
                }
                println!("Synchronizing after gamestate computation");
                computed_gamestate.wait();
                println!("Synchronized");
                // broadcast count
                stream
                    .write(format!("{}\n", *counter.lock().unwrap()).as_bytes())
                    .expect("Could not write to the stream");
            },
            ConnectionType::Spectator => {
                println!("Synchronizing after gamestate computation");
                computed_gamestate.wait();
                println!("Synchronized");
                // broadcast count
                stream
                    .write(format!("{}\n", *counter.lock().unwrap()).as_bytes())
                    .expect("Could not write to the stream");
            }
        }
    }
}

fn main() {
    let listener = TcpListener::bind("127.0.0.1:44556").unwrap();
    let counter: Arc<Mutex<i32>> = Arc::new(Mutex::new(0));
    let num_competitors: Arc<Mutex<u32>> = Arc::new(Mutex::new(0));
    let num_spectators = Arc::new(Mutex::new(0));
    // must recieve input from both threads before computing gamestate
    let recieved_inputs = Arc::new(Barrier::new(2));
    // competitor and spectator handling threads must wait for new gamestate to be computed
    let computed_gamestate = Arc::new(Barrier::new(3));
    let mut threads_spawned: u32 = 0;
    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                println!("Connected to client!");
                let counter = Arc::clone(&counter);
                let num_competitors = Arc::clone(&num_competitors);
                let has_spectator = Arc::clone(&num_spectators);
                let recieved_inputs = Arc::clone(&recieved_inputs);
                let computed_gamestate = Arc::clone(&computed_gamestate);
                if threads_spawned < 2 {
                    thread::spawn(move || {
                        handle_client(stream, counter,num_competitors, has_spectator,
                            recieved_inputs, computed_gamestate);
                    });
                    threads_spawned += 1;
                } else {
                    // use the main thread to handle the third connection to reduce thread usage
                    handle_client(stream, counter,num_competitors, has_spectator,
                        recieved_inputs, computed_gamestate);
                }
            }
            Err(e) => {
                eprintln!("Unable to connect: {}", e);
            }
        }
    }
}

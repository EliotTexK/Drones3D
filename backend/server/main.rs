use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex, Barrier};
use std::thread;
use std::time::{Instant, Duration};

use clap::Parser;


// TCP buffer size for incoming client messages
const BUFFER_SIZE: usize = 512;

enum ConnectionType {
    Unknown,
    CompetitorA,
    CompetitorB,
    Spectator
}

fn read_until_newline(stream: &mut TcpStream) -> String {
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

    // Attempt to read up to delimiter
    if let Some(newline_index) = buf.iter().position(|&x| x == b'\n') {
        return String::from_utf8_lossy(&buf[..newline_index]).to_string();
    } else {
        // client messages should have a known maximum size, and since clients/server
        // are synchronized, we shouldn't worry about clients sending multiple messages
        // before the server is ready to recieve them. If the buffer overflows, then
        // the client is at fault.
        return String::from("ERR");
    }
}

fn handle_client(
    mut stream: TcpStream,
    counter: Arc<Mutex<i32>>,
    num_competitors: Arc<Mutex<u32>>,
    num_spectators: Arc<Mutex<u32>>,
    recieved_inputs: Arc<Barrier>,
    computed_gamestate: Arc<Barrier>,
    training_mode: bool,
    frame_delay: u64,
    max_timeout_deficit: u128
) {
    let mut connection_type: ConnectionType = ConnectionType::Unknown;
    let mut last_broadcast = Instant::now();
    let mut timeout_deficit: u128 = 0;

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
                        last_broadcast = Instant::now();
                    },
                    "SPECTATOR" => {
                        if training_mode {
                            println!("Training mode, no spectators allowed: disconnect");
                            return;
                        }
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
                if !training_mode {
                    // measure time since last broadcast
                    let since_last_broadcast = last_broadcast.elapsed();
                    let remaining_sleep_time =
                        Duration::from_millis(frame_delay) - since_last_broadcast;
                    // Check if sleeping is needed
                    if remaining_sleep_time > Duration::from_millis(0) {
                        // Sleep until next frame
                        std::thread::sleep(remaining_sleep_time);
                    }
                    if remaining_sleep_time < Duration::from_millis(0) {
                        // Add to timeout deficit
                        timeout_deficit -= remaining_sleep_time.as_millis();
                        if timeout_deficit > max_timeout_deficit {
                            println!("Competitor has taken too long: disconnect");
                            return;
                        }
                    }
                }
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
                last_broadcast = Instant::now();
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

#[derive(Parser, Debug)]
#[clap(author="Eliot Kimmel", version, about="Backend game server demo")]
struct Args {
    /// Run in training mode: headless with maximum possible framerate
    #[arg(short, long, default_value_t = false)]
    training_mode: bool,
    /// Frame delay in milliseconds, for normal mode
    #[arg(short, long, default_value_t = 16)]
    frame_delay: u64,
    /// Maximum total milliseconds a competitor can be "late" with their response
    #[arg(short, long, default_value_t = 1000)]
    max_timeout_deficit: u128
}


fn main() {
    let args = Args::parse();
    let training_mode = args.training_mode;
    let frame_delay = args.frame_delay;
    let max_timeout_deficit = args.max_timeout_deficit;
    let listener = TcpListener::bind("127.0.0.1:44556").unwrap();
    let counter: Arc<Mutex<i32>> = Arc::new(Mutex::new(0));
    let num_competitors: Arc<Mutex<u32>> = Arc::new(Mutex::new(0));
    let num_spectators = Arc::new(Mutex::new(0));
    // must recieve input from both threads before computing gamestate
    let recieved_inputs = Arc::new(Barrier::new(2));
    // competitor and spectator handling threads must wait for new gamestate to be computed
    // don't sync with spectator thread in training mode
    let computed_gamestate = Arc::new(Barrier::new(2 + if training_mode {0} else {1}));
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
                // need 1 handler thread in training mode, need 2 otherwise
                if threads_spawned < if training_mode {1} else {2} {
                    thread::spawn(move || {
                        handle_client(stream, counter,num_competitors, has_spectator,
                            recieved_inputs, computed_gamestate, training_mode, frame_delay,
                            max_timeout_deficit
                        );
                    });
                    threads_spawned += 1;
                } else {
                    // use the main thread to handle final connection to reduce thread usage
                    handle_client(stream, counter,num_competitors, has_spectator,
                        recieved_inputs, computed_gamestate, training_mode, frame_delay,
                        max_timeout_deficit
                    );
                }
            }
            Err(e) => {
                eprintln!("Unable to connect: {}", e);
            }
        }
    }
}

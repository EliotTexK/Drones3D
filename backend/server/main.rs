use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex, Barrier};
use std::thread;
use std::time::{Instant, Duration};
use clap::Parser;
use gamestate::Gamestate;
use rand::thread_rng;

pub mod gamestate;

// TCP buffer size for incoming client messages
const BUFFER_SIZE: usize = 1024;

enum ConnectionType {
    Unknown,
    CompetitorA,
    CompetitorB,
    Spectator
}

// pass in buffer so we don't have to keep re-declaring it
fn read_until_newline(stream: &mut TcpStream, buf: &mut [u8;BUFFER_SIZE]) -> String {
    // Read bytes into the buffer
    println!("Reading bytes");
    let bytes_read = stream
        .read(buf)
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
    num_competitors: Arc<Mutex<u32>>,
    num_spectators: Arc<Mutex<u32>>,
    recieved_inputs: Arc<Barrier>,
    computed_next_tick: Arc<Barrier>,
    gamestate: Arc<Mutex<Gamestate>>,
    input_a: Arc<Mutex<String>>,
    input_b: Arc<Mutex<String>>,
    training_mode: bool,
    game_tick_delay: u64,
    competitor_max_debt: u128,
) {
    let mut connection_type: ConnectionType = ConnectionType::Unknown;
    let mut last_broadcast = Instant::now();
    let mut timeout_debt: u128 = 0;
    let mut buf: [u8;BUFFER_SIZE] = [0;BUFFER_SIZE];
    let mut rng = thread_rng();

    loop {

        match connection_type {
            // Handle client identification
            ConnectionType::Unknown => {
                let msg = read_until_newline(&mut stream, &mut buf);
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
                        let gamestate = gamestate.lock().unwrap();
                        stream
                        .write(format!("{}\n", serde_json::to_string(&*gamestate).unwrap()).as_bytes())
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
                        let gamestate = gamestate.lock().unwrap();
                        stream
                        .write(format!("{}\n", serde_json::to_string(&*gamestate).unwrap()).as_bytes())
                            .expect("Could not write to the stream");
                        println!("Broadcasted initial gamestate");
                    },
                    _ => {
                        println!("Client failed to identify itself: disconnect");
                        return;
                    }
                }
            },
            ConnectionType::CompetitorA => {
                {
                    let mut input_a = input_a.lock().unwrap();
                    *input_a = read_until_newline(&mut stream, &mut buf);
                }
                if !training_mode {
                    // measure time since last broadcast
                    let since_last_broadcast = last_broadcast.elapsed();
                    let remaining_sleep_time =
                        Duration::from_millis(game_tick_delay) - since_last_broadcast;
                    // Check if sleeping is needed
                    if remaining_sleep_time > Duration::from_millis(0) {
                        // Sleep until next game tick
                        std::thread::sleep(remaining_sleep_time);
                    }
                    if remaining_sleep_time < Duration::from_millis(0) {
                        // Add to timeout deficit
                        timeout_debt -= remaining_sleep_time.as_millis();
                        if timeout_debt > competitor_max_debt {
                            println!("Competitor has taken too long: disconnect");
                            return;
                        }
                    }
                }
                // wait for the other competitor to send data
                println!("Waiting for both inputs to be recieved");
                recieved_inputs.wait();
                println!("Recieved both inputs");
                // compute gamestate in Competitor A's handler thread
                // the distiction between thread A and B is arbitrary
                {
                    let input_b = input_b.lock().unwrap();
                    let input_a = input_a.lock().unwrap();
                    let mut gamestate = gamestate.lock().unwrap();
                    gamestate.compute_next_tick(&mut rng, input_a.clone(), input_b.clone());
                }
                println!("Competitor A: synchronizing after gamestate computation");
                computed_next_tick.wait();
                println!("Synchronized");
                // broadcast game state
                stream
                    .write(format!("{}\n", serde_json::to_string(&*gamestate).unwrap()).as_bytes())
                    .expect("Could not write to the stream");
                last_broadcast = Instant::now();
            },
            ConnectionType::CompetitorB => {
                {
                    let mut input_b = input_b.lock().unwrap();
                    *input_b = read_until_newline(&mut stream, &mut buf);
                }
                // TODO: make the following lines into a function
                if !training_mode {
                    // measure time since last broadcast
                    let since_last_broadcast = last_broadcast.elapsed();
                    let remaining_sleep_time =
                        Duration::from_millis(game_tick_delay) - since_last_broadcast;
                    // Check if sleeping is needed
                    if remaining_sleep_time > Duration::from_millis(0) {
                        // Sleep until next game tick
                        std::thread::sleep(remaining_sleep_time);
                    }
                    if remaining_sleep_time < Duration::from_millis(0) {
                        // Add to timeout deficit
                        timeout_debt -= remaining_sleep_time.as_millis();
                        if timeout_debt > competitor_max_debt {
                            println!("Competitor has taken too long: disconnect");
                            return;
                        }
                    }
                }
                // wait for the other competitor to send data
                println!("Waiting for both inputs to be recieved");
                recieved_inputs.wait();
                println!("Recieved both inputs");
                // competitor A computes
                println!("Competitor B: synchronizing after gamestate computation");
                computed_next_tick.wait();
                println!("Synchronized");
                // broadcast game state
                let gamestate = gamestate.lock().unwrap();
                stream
                .write(format!("{}\n", serde_json::to_string(&*gamestate).unwrap()).as_bytes())
                    .expect("Could not write to the stream");
                last_broadcast = Instant::now();
            },
            ConnectionType::Spectator => {
                println!("Spectator: synchronizing after gamestate computation");
                computed_next_tick.wait();
                println!("Synchronized");
                // broadcast game state
                let gamestate = gamestate.lock().unwrap();
                stream
                .write(format!("{}\n", serde_json::to_string(&*gamestate).unwrap()).as_bytes())
                    .expect("Could not write to the stream");
                last_broadcast = Instant::now();
            }
        }
    }
}

#[derive(Parser, Debug)]
#[clap(author="Eliot Kimmel", version, about="Backend game server demo")]
struct Args {
    /// Run in training mode: headless with maximum possible game tick rate
    #[arg(short, long, default_value_t = false)]
    training_mode: bool,
    /// Game tick delay in milliseconds, for normal mode
    #[arg(short, long, default_value_t = 16)]
    game_tick_delay: u64,
    /// Maximum total milliseconds a competitor can be "late" with their response
    #[arg(short, long, default_value_t = 1000)]
    competitor_max_debt: u128,
    /// Maximum game ticks until the game is over
    #[arg(short, long, default_value_t = 10000)]
    max_game_ticks: u32
}


fn main() {
    let args = Args::parse();
    let training_mode = args.training_mode;
    let game_tick_delay = args.game_tick_delay;
    let competitor_max_debt = args.competitor_max_debt;
    let max_game_ticks = args.max_game_ticks;
    let listener = TcpListener::bind("127.0.0.1:44556").unwrap();
    let num_competitors: Arc<Mutex<u32>> = Arc::new(Mutex::new(0));
    let num_spectators = Arc::new(Mutex::new(0));
    let gamestate = Arc::new(Mutex::new(Gamestate::new(&mut thread_rng(), max_game_ticks)));
    let input_a = Arc::new(Mutex::new(String::new()));
    let input_b = Arc::new(Mutex::new(String::new()));
    // must recieve input from both threads before computing gamestate
    let recieved_inputs = Arc::new(Barrier::new(2));
    // competitor and spectator handling threads must wait for new gamestate to be computed
    // don't sync with spectator thread in training mode
    let computed_next_tick = Arc::new(Barrier::new(2 + if training_mode {0} else {1}));
    let mut threads_spawned: u32 = 0;
    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                println!("Connected to client!");
                let num_competitors = Arc::clone(&num_competitors);
                let has_spectator = Arc::clone(&num_spectators);
                let recieved_inputs = Arc::clone(&recieved_inputs);
                let computed_next_tick = Arc::clone(&computed_next_tick);
                let gamestate = Arc::clone(&gamestate);
                let input_a = Arc::clone(&input_a);
                let input_b = Arc::clone(&input_b);
                // spawn 1 handler thread in training mode, spawn 2 otherwise
                if threads_spawned < if training_mode {1} else {2} {
                    thread::spawn(move || {
                        handle_client(stream,num_competitors, has_spectator,
                            recieved_inputs, computed_next_tick, gamestate, input_a, input_b,
                            training_mode, game_tick_delay, competitor_max_debt
                        );
                    });
                    threads_spawned += 1;
                } else {
                    // use the main thread to handle final connection to reduce thread usage
                    handle_client(stream, num_competitors, has_spectator,
                        recieved_inputs, computed_next_tick, gamestate, input_a, input_b,
                        training_mode, game_tick_delay, competitor_max_debt
                    );
                }
            }
            Err(e) => {
                eprintln!("Unable to connect: {}", e);
            }
        }
    }
}

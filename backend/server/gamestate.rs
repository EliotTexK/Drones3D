use std::{collections::HashMap, ops::Range};
use rand::{Rng, rngs::ThreadRng};

// arena shaped like a cube
const ARENA_SIZE: f32 = 20.0;
const PLAYER_SPAWN_RANGE: Range<f32> = Range {
    start: ARENA_SIZE * 0.75,
    end: ARENA_SIZE * 0.25,
};
const PLAYER_RADIUS: f32 = 1.0;
const AMMO_MAX: u32 = 3;
const RELOAD_TIMER_MAX: u32 = 60;
const FIRE_RATE_TIMER_MAX: u32 = 10;
const MAX_OBSTACLE_SPAWN_TIMER: u32 = 80;
const OBSTACLE_SPAWN_TIMER_INIT: Range<u32> = Range {
    start: 0,
    end: 20
};
const OBSTACLE_SPAWN_DISTANCE: f32 = ARENA_SIZE * 1.5;
const OBSTACLE_RADIUS: Range<f32> = Range {
    start: 1.5,
    end: 2.0
};
const BULLET_LENGTH: f32 = 0.5;

// float input restricted between -1 and 1
struct Threshold {
    value: f32,
}

impl Threshold {
    pub fn new(value: f32) -> Threshold {
        Threshold { value: value.clamp(-1.0, 1.0) }
    }
}

struct Controls {
    rot_y: Threshold,
    forward_back: Threshold,
    up_down: Threshold,
    shoot: bool
}

impl Controls {
    pub fn deserialize(input: String) -> Controls {
        return Controls {
            rot_y: Threshold::new(1.0),
            forward_back: Threshold::new(1.0),
            up_down: Threshold::new(1.0),
            shoot: false
        }
    }
}

struct Player {
    position: [f32;3],
    velocity: [f32;3],
    rot_y: f32, // radians
    ammo: u32,
    reload_timer: u32, // game ticks
    fire_rate_timer: u32, // game ticks
    is_dead: bool,
    respawn_timer: u32, // game_ticks
}

impl Player {
    // TODO: need spawn function that avoids obstacles and samples a
    //       spawnpoint away from adversaries and close to teammates
    pub fn spawn(rng: &mut ThreadRng) -> Player {
        Player {
            position: [
                rng.gen_range(PLAYER_SPAWN_RANGE),
                rng.gen_range(PLAYER_SPAWN_RANGE),
                rng.gen_range(PLAYER_SPAWN_RANGE)
            ],
            velocity: [0.0,0.0,0.0],
            rot_y: 0.0,
            ammo: AMMO_MAX,
            reload_timer: 0,
            fire_rate_timer: 0,
            is_dead: false,
            respawn_timer: 0
        }
    }
}

struct Obstacle {
    guid: u64,
    position: [f32;3],
    radius: f32,
    velocity: [f32;3]
}

struct Bullet {
    guid: u64,
    position: [f32;3],
    velocity: [f32;3]
}

struct Gamestate {
    obstacles: HashMap<u64,Obstacle>,
    obstacle_counter: u64,
    obstacle_spawn_timer: u32,
    bullets: HashMap<u64,Bullet>,
    bullet_counter: u64,
    player_a1: Player,
    player_a2: Player,
    player_b1: Player,
    player_b2: Player,
    score_a: u32,
    score_b: u32,
    ticks_progressed: u32,
    max_game_ticks: u32
}

fn sphere_inside_central_cube(
    position: [f32;3],
    radius: f32,
    half_side_length: f32
) -> bool {
    // TODO: implement
    return false;
}

impl Gamestate {
    pub fn new(rng: &mut ThreadRng, max_game_ticks: u32) -> Gamestate {
        Gamestate {
            obstacles: HashMap::new(),
            obstacle_counter: 0,
            obstacle_spawn_timer: rng.gen_range(OBSTACLE_SPAWN_TIMER_INIT),
            bullets: HashMap::new(),
            bullet_counter: 0,
            player_a1: Player::spawn(rng),
            player_a2: Player::spawn(rng),
            player_b1: Player::spawn(rng),
            player_b2: Player::spawn(rng),
            score_a: 0,
            score_b: 0,
            ticks_progressed: 0,
            max_game_ticks: max_game_ticks
        }
    }
    pub fn compute_next_tick(
        &mut self, rng: &mut ThreadRng,
        controls_a: &Controls, controls_b: &Controls
    ) {
        
    }
}
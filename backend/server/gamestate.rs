use std::{collections::HashMap, ops::Range};
use rand::{Rng, rngs::ThreadRng};
use serde::{Serialize, Deserialize};

// game area bounded by a cube, this value is half of said cube's side length
const GAME_AREA_SIZE: f32 = 20.0;
// area outside of which obstacles are deleted
const OBSTACLE_AREA_SIZE: f32 = GAME_AREA_SIZE * 1.5;
// don't spawn too close to the edge, or too close to the center
const PLAYER_SPAWN_RANGE: Range<f32> = Range {
    start: GAME_AREA_SIZE * 0.25,
    end: GAME_AREA_SIZE * 0.75,
};
const PLAYER_RADIUS: f32 = 1.0;
const MAX_PLAYER_RESPAWN_TIMER: u32 = 80;
const PLAYER_THRUST_FACTOR: f32 = 0.1;
const PLAYER_TURN_SPEED: f32 = 0.1; // radians per game tick 
const AMMO_MAX: u32 = 3;
const RELOAD_TIMER_MAX: u32 = 60;
const FIRE_RATE_TIMER_MAX: u32 = 10;
const MAX_OBSTACLE_SPAWN_TIMER: u32 = 80;
const OBSTACLE_SPAWN_TIMER_INIT: Range<u32> = Range {
    start: 0,
    end: 20
};
const OBSTACLE_RADIUS: Range<f32> = Range {
    start: 1.5,
    end: 2.0
};
const BULLET_SPEED: f32 = 0.25;

fn get_random_spawnpoint(rng: &mut ThreadRng) -> [f32;3] {
    return [
        rng.gen_range(PLAYER_SPAWN_RANGE),
        rng.gen_range(PLAYER_SPAWN_RANGE),
        rng.gen_range(PLAYER_SPAWN_RANGE)
    ];
}

fn intersect_sphere_lineseg(
    center: &[f32;3],
    radius: &f32,
    v1: &[f32;3],
    v2: &[f32;3]
) -> bool {
    // solved using parametric equation for a line: l(t) = v1*(1-t) + v2
    // we plug this into the sphere's equation x^2 + y^2 + z^2 = r^2
    // this yields a quadratic which we solve for t
    // we have intersection if at least one solution satisfies 0 < t < 1
    let r2 = radius.powf(2.0);
    let a = (v1[0]-center[0]).powf(2.0) +
            (v1[1]-center[1]).powf(2.0) +
            (v1[2]-center[2]).powf(2.0) - r2;
    if a == 0.0 { return true; } // edge case (literally)
    let c = (v1[0]-v2[0]).powf(2.0) +
            (v1[1]-v2[1]).powf(2.0) +
            (v1[2]-v2[2]).powf(2.0);
    let b = (v2[0]-center[0]).powf(2.0) +
            (v2[1]-center[1]).powf(2.0) +
            (v2[2]-center[2]).powf(2.0) - a - c - r2;
    let b2 = b.powf(2.0);
    let t_min = (-b - (b2 - 4.0*a*c).sqrt())/2.0*a;
    let t_max = (-b - (b2 - 4.0*a*c).sqrt())/2.0*a;
    return (0.0 < t_min && t_min < 1.0) || (0.0 < t_max && t_max < 1.0);
}

fn intersect_spheres(
    center_1: &[f32;3],
    radius_1: &f32,
    center_2: &[f32;3],
    radius_2: &f32
) -> bool {
    let distance = f32::sqrt(
        (center_2[0] - center_1[0]) * (center_2[0] - center_1[0]) +
        (center_2[1] - center_1[1]) * (center_2[1] - center_1[1]) +
        (center_2[2] - center_1[2]) * (center_2[2] - center_1[2])

    );
    return distance < radius_1 + radius_2;
}

fn add_vec3(v1: &mut [f32;3], v2: &[f32;3]) {
    v1[0] += v2[0];
    v1[1] += v2[1];
    v1[2] += v2[2];
}

// Used for despawning obstacles/bullets: exact collision not necessary
fn inside_obstacle_area(
    position: &[f32;3]
) -> bool {
    return position[0] >= -OBSTACLE_AREA_SIZE
        && position[0] <= OBSTACLE_AREA_SIZE
        && position[1] >= -OBSTACLE_AREA_SIZE
        && position[1] <= OBSTACLE_AREA_SIZE
        && position[2] >= -OBSTACLE_AREA_SIZE
        && position[2] <= OBSTACLE_AREA_SIZE
}

fn inside_game_area(
    position: &[f32;3],
    radius: &f32,
) -> bool {
    return (position[0] - radius) >= -GAME_AREA_SIZE
        && (position[0] + radius) <= GAME_AREA_SIZE
        && (position[1] - radius) >= -GAME_AREA_SIZE
        && (position[1] + radius) <= GAME_AREA_SIZE
        && (position[2] - radius) >= -GAME_AREA_SIZE
        && (position[2] + radius) <= GAME_AREA_SIZE;
}

#[derive(PartialEq, Eq, Hash, Clone, Copy, Serialize)]
enum Team {
    A,
    B
}

#[derive(Deserialize)]
struct ControlsRaw {
    rot_y: f32,
    forward_back: f32,
    up_down: f32,
    shoot: bool
}

#[derive(Deserialize)]
pub struct InputRaw {
    controls_1: ControlsRaw,
    controls_2: ControlsRaw
}

#[derive(Clone, Copy)]
struct Controls {
    rot_y: f32,
    forward_back: f32,
    up_down: f32,
    shoot: bool
}

impl Controls {
    fn validate_raw_controls(raw: &InputRaw) -> [Controls;2] {
        [
            Controls {
                rot_y: raw.controls_1.rot_y.clamp(-1.0,1.0),
                forward_back: raw.controls_1.forward_back.clamp(-1.0,1.0),
                up_down: raw.controls_1.up_down.clamp(-1.0, 1.0),
                shoot: raw.controls_1.shoot
            },
            Controls {
                rot_y: raw.controls_2.rot_y.clamp(-1.0,1.0),
                forward_back: raw.controls_2.forward_back.clamp(-1.0,1.0),
                up_down: raw.controls_2.up_down.clamp(-1.0, 1.0),
                shoot: raw.controls_2.shoot
            }
        ]
    }
    fn empty() -> Controls {
        Controls {
            rot_y: 0.0,
            forward_back: 0.0,
            up_down: 0.0,
            shoot: false
        }
    }
}

#[derive(Serialize)]
struct Player {
    team: Team,
    position: [f32;3],
    velocity: [f32;3],
    rot_y: f32, // radians
    ammo: u32,
    reload_timer: u32, // game ticks
    fire_rate_timer: u32, // game ticks
    is_dead: bool,
    respawn_timer: u32 // game_ticks
}

impl Player {
    // TODO: implement find_fair_spawnpoint
    pub fn spawn(position: [f32;3], team: Team) -> Player {
        Player {
            team: team,
            position: position,
            velocity: [0.0,0.0,0.0],
            rot_y: 0.0,
            ammo: AMMO_MAX,
            reload_timer: 0,
            fire_rate_timer: 0,
            is_dead: false,
            respawn_timer: 0
        }
    }
    // TODO: figure out a way to do this with less lines of code
    pub fn respawn(&mut self, rng: &mut ThreadRng) {
        self.position = [
            rng.gen_range(PLAYER_SPAWN_RANGE),
            rng.gen_range(PLAYER_SPAWN_RANGE),
            rng.gen_range(PLAYER_SPAWN_RANGE)
        ];
        self.velocity = [0.0,0.0,0.0];
        self.rot_y = 0.0;
        self.ammo = AMMO_MAX;
        self.reload_timer = 0;
        self.fire_rate_timer = 0;
        self.is_dead = false;
        self.respawn_timer = 0;
    }
}


#[derive(Serialize)]
struct Obstacle {
    guid: u64,
    position: [f32;3],
    radius: f32,
    velocity: [f32;3]
}

// bullet is a line segment: between its current and previous position
#[derive(Serialize)]
struct Bullet {
    team: Team,
    guid: u64,
    position: [f32;3],
    prev_position: [f32;3],
    velocity: [f32;2] // only moves horizontally, for now
}

#[derive(Serialize)]
pub struct Gamestate {
    ticks_progressed: u32,
    max_game_ticks: u32,
    obstacles: HashMap<u64,Obstacle>,
    obstacle_counter: u64,
    obstacle_spawn_timer: u32,
    bullets: HashMap<u64,Bullet>,
    bullet_counter: u64,
    player_a1: Player,
    player_a2: Player,
    player_b1: Player,
    player_b2: Player,
    scores: HashMap<Team, i32>
}

impl Gamestate {
    pub fn find_fair_spawnpoint(&self, rng: &mut ThreadRng) -> [f32;3] {
        // TODO: find spawnpoint devoid of players/obstacles/bullets
        return [
            rng.gen_range(PLAYER_SPAWN_RANGE),
            rng.gen_range(PLAYER_SPAWN_RANGE),
            rng.gen_range(PLAYER_SPAWN_RANGE)
        ];
    }
    pub fn new(rng: &mut ThreadRng, max_game_ticks: u32) -> Gamestate {
        let mut retval = Gamestate {
            ticks_progressed: 0,
            max_game_ticks: max_game_ticks,
            obstacles: HashMap::new(),
            obstacle_counter: 0,
            obstacle_spawn_timer: rng.gen_range(OBSTACLE_SPAWN_TIMER_INIT),
            bullets: HashMap::new(),
            bullet_counter: 0,
            player_a1: Player::spawn([0.0,0.0,0.0],Team::A),
            player_a2: Player::spawn([0.0,0.0,0.0], Team::A),
            player_b1: Player::spawn([0.0,0.0,0.0], Team::B),
            player_b2: Player::spawn([0.0,0.0,0.0], Team::B),
            scores: HashMap::from([
                (Team::A, 0),
                (Team::B, 0)
            ])
        };
        retval.player_a1.position = get_random_spawnpoint(rng);
        retval.player_a2.position = retval.find_fair_spawnpoint(rng);
        retval.player_b1.position = retval.find_fair_spawnpoint(rng);
        retval.player_b2.position = retval.find_fair_spawnpoint(rng);
        return retval;
    }
    pub fn compute_next_tick(
        &mut self, rng: &mut ThreadRng,
        input_a: String,
        input_b: String
    ) {
        let controls_a1: Controls;
        let controls_a2: Controls;
        let controls_b1: Controls;
        let controls_b2: Controls;
        // validate controls sent by user
        {
            let result_a: Result<InputRaw,serde_json::Error> = serde_json::from_str(&input_a);
            let controls_a: [Controls;2] = match result_a {
                Ok(raw) => {
                    Controls::validate_raw_controls(&raw)
                },
                Err(_) => {
                    [Controls::empty(), Controls::empty()]
                },
            };
            controls_a1 = controls_a[0];
            controls_a2 = controls_a[1];
            let result_b: Result<InputRaw,serde_json::Error> = serde_json::from_str(&input_b);
            let controls_b: [Controls;2] = match result_b {
                Ok(raw) => {
                    Controls::validate_raw_controls(&raw)
                },
                Err(_) => {
                    [Controls::empty(), Controls::empty()]
                },
            };
            controls_b1 = controls_b[0];
            controls_b2 = controls_b[1];
        }
        // tick main game timer
        self.ticks_progressed += 1;
        // spawn, move, and despawn obstacles
        self.obstacle_spawn_timer += 1;
        if self.obstacle_spawn_timer > MAX_OBSTACLE_SPAWN_TIMER {
            let spawned = Obstacle {
                guid: self.obstacle_counter,
                // TODO: spawn position should be outside the game area, but inside the obstacle area
                position: get_random_spawnpoint(rng),
                // TODO: velocity should point the obstacle towards somewhere in the game area
                velocity: [0.0,0.0,0.0],
                radius: rng.gen_range(OBSTACLE_RADIUS)
            };
            self.obstacles.insert(spawned.guid, spawned);
            self.obstacle_counter += 1;
            self.obstacle_spawn_timer = rng.gen_range(OBSTACLE_SPAWN_TIMER_INIT);
        }
        {
            let mut obstacles_to_delete: Vec<u64> = vec![];
            for kv in &mut self.obstacles.iter_mut() {
                // add velocities to positions for obstacles
                let obstacle = kv.1;
                add_vec3(&mut obstacle.position, &obstacle.velocity);
                // delete when out-of-bounds
                if !inside_obstacle_area(&obstacle.position) {
                    obstacles_to_delete.push(obstacle.guid);
                }   
            }
            for to_delete in obstacles_to_delete {
                self.obstacles.remove(&to_delete);
            }
        }
        // player movement, shooting and respawn logic
        for player_controls in [
            (&mut self.player_a1, &controls_a1),
            (&mut self.player_a2, &controls_a2),
            (&mut self.player_b1, &controls_b1),
            (&mut self.player_b2, &controls_b2)
        ] {
            let player = player_controls.0;
            let controls = *player_controls.1;
            if player.is_dead {
                if player.respawn_timer <= MAX_PLAYER_RESPAWN_TIMER {
                    player.respawn_timer += 1;
                    continue; // player is dead, don't bother with other logic
                } else {
                    // respawn player
                    player.respawn(rng);
                }
            }
            if player.fire_rate_timer <= FIRE_RATE_TIMER_MAX {
                player.fire_rate_timer += 1;
            }
            if player.reload_timer <= RELOAD_TIMER_MAX {
                player.reload_timer += 1;
            } else {
                if player.ammo < AMMO_MAX {
                    player.ammo += 1;
                    player.reload_timer = 0;
                }
            }
            player.velocity[2] += controls.up_down * PLAYER_THRUST_FACTOR;
            player.rot_y += controls.rot_y * PLAYER_TURN_SPEED;
            let direction_vec2 = [
                f32::cos(player.rot_y),
                f32::sin(player.rot_y)
            ];
            player.velocity[0] += direction_vec2[0] * PLAYER_THRUST_FACTOR * controls.forward_back;
            player.velocity[1] += direction_vec2[1] * PLAYER_THRUST_FACTOR * controls.forward_back;
            add_vec3(&mut player.position, &player.velocity);
            if controls.shoot && player.ammo > 0
            && player.fire_rate_timer > FIRE_RATE_TIMER_MAX {
                let new_pos = [
                    player.position[0] + PLAYER_RADIUS * 1.5 * direction_vec2[0] ,
                    player.position[1] + PLAYER_RADIUS * 1.5 * direction_vec2[1],
                    player.position[2]
                ];
                let spawned = Bullet {
                    guid: self.bullet_counter,
                    team: player.team,
                    position: new_pos,
                    prev_position: new_pos,
                    velocity: [
                        direction_vec2[0] * BULLET_SPEED,
                        direction_vec2[1] * BULLET_SPEED
                    ],
                };
                self.bullets.insert(spawned.guid, spawned);
                self.bullet_counter += 1;
                player.fire_rate_timer = 0;
                player.ammo -= 1;
            }
        }
        // move and despawn bullets
        {
            let mut bullets_to_delete: Vec<u64> = vec![];
            for kv in &mut self.bullets.iter_mut() {
                // add velocities to positions for bullets
                let bullet = kv.1;
                bullet.position[0] += bullet.velocity[0];
                bullet.position[1] += bullet.velocity[1];
                // delete when out-of-bounds
                if !inside_obstacle_area(&bullet.position) {
                    bullets_to_delete.push(bullet.guid);
                }   
            }
            for to_delete in bullets_to_delete {
                self.bullets.remove(&to_delete);
            }
        }
        // collide players with each other
        for &i in &[0, 1, 2, 3, 4, 5] {
            let (player_1, player_2) = match i {
                0 => (&mut self.player_a1, &mut self.player_a2),
                1 => (&mut self.player_a1, &mut self.player_b1),
                2 => (&mut self.player_a1, &mut self.player_b2),
                3 => (&mut self.player_a2, &mut self.player_b1),
                4 => (&mut self.player_a2, &mut self.player_b2),
                5 => (&mut self.player_b1, &mut self.player_b2),
                _ => unreachable!(),
            };

            if intersect_spheres(
                &player_1.position,
                &PLAYER_RADIUS,
                &player_2.position,
                &PLAYER_RADIUS,
            ) {
                player_1.is_dead = true;
                player_2.is_dead = true;
                player_1.respawn_timer = 0;
                player_2.respawn_timer = 0;
            }
        }
        // collide players with obstacles
        for player in [
            &mut self.player_a1,
            &mut self.player_a2,
            &mut self.player_b1,
            &mut self.player_b2
        ] {
            for obstacle in self.obstacles.values() {
                if intersect_spheres(
                    &player.position,
                    &PLAYER_RADIUS,
                    &obstacle.position,
                    &obstacle.radius
                ) {
                    player.is_dead = true;
                    player.respawn_timer = 0;
                    *self.scores.get_mut(&player.team).unwrap() -= 1;
                }
            }
        }
        // collide players with bullets
        for player in [
            &mut self.player_a1,
            &mut self.player_a2,
            &mut self.player_b1,
            &mut self.player_b2
        ] {
            for bullet in self.bullets.values() {
                if intersect_sphere_lineseg(
                    &player.position,
                    &PLAYER_RADIUS,
                    &bullet.position,
                    &bullet.prev_position
                ) {
                    player.is_dead = true;
                    player.respawn_timer = 0;
                    // don't award points for friendly-fire
                    if bullet.team != player.team {
                        *self.scores.get_mut(&bullet.team).unwrap() += 2;
                    }
                    *self.scores.get_mut(&player.team).unwrap() -= 1;
                }
            }
        }
        // kill players that are out-of-bounds
        for player in [
            &mut self.player_a1,
            &mut self.player_a2,
            &mut self.player_b1,
            &mut self.player_b2
        ] {
            if inside_game_area(&player.position, &PLAYER_RADIUS) {
                player.is_dead = true;
                player.respawn_timer = 0;
            }
        }
    }
}
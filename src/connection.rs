use std::net::{IpAddr, Ipv4Addr, SocketAddr};
extern crate serde;
use crate::snake::Direction;
use self::serde::{Deserialize, Serialize};
extern crate tokio;
use std::{io, thread};
use tokio::net::{UdpSocket};
use tokio::time::{Duration, Instant};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename = "NodeRole")]
enum NodeRole {
    NORMAL = 0,
    MASTER = 1,
    DEPUTY = 2,
    VIEWER = 3,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "UPPERCASE")]
enum PlayerType {
    HUMAN = 0,
    ROBOT = 1,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct GamePlayer {
    name: String,
    id: u64,
    #[serde(default)]
    ip_address: Option<String>,
    #[serde(default)]
    port: Option<u64>,
    role: NodeRole,
    #[serde(default = "default_player_type")]
    #[serde(rename = "type")]
    player_type: PlayerType,
    score: u64,
}

fn default_player_type() -> PlayerType {
    PlayerType::HUMAN
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct GameConfig {
    #[serde(default = "default_width")]
    pub width: u64,
    #[serde(default = "default_height")]
    pub height: u64,
    #[serde(default = "default_food_static")]
    pub food_static: u64,
    #[serde(default = "default_state_delay_ms")]
    pub state_delay_ms: u64,
}

impl Default for GameConfig {
    fn default() -> Self {
        GameConfig{
            width:default_width(),
            height: default_height(),
            food_static: default_food_static(),
            state_delay_ms: default_state_delay_ms(),
        }
    }
}

fn default_width() -> u64 {
    40
}

fn default_height() -> u64 {
    30
}

fn default_food_static() -> u64 {
    1
}

fn default_state_delay_ms() -> u64 {
    1000
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct GamePlayers {
    players: Vec<GamePlayer>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct Coord {
    #[serde(default)]
    x: u64,
    #[serde(default)]
    y: u64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "UPPERCASE")]
enum SnakeState {
    ALIVE = 0,
    ZOMBIE = 1,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct Snake {
    player_id: u64,
    points: Vec<Coord>,
    #[serde(default = "default_snake_state")]
    state: SnakeState,
    head_direction: Direction,
}

fn default_snake_state() -> SnakeState {
    SnakeState::ALIVE
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct GameState {
    state_order: u64,
    snakes: Vec<Snake>,
    foods: Vec<Coord>,
    players: GamePlayers,
}

#[derive(Debug, Serialize, Deserialize)]
struct GameAnnouncement {
    players: GamePlayers,
    config: GameConfig,
    #[serde(default = "default_can_join")]
    can_join: bool,
    game_name: String,
}

fn default_can_join() -> bool {
    true
}

#[derive(Debug, Serialize, Deserialize)]
enum GameMessageType {
    PingMsg,
    SteerMsg { direction: Direction },
    AckMsg,
    StateMsg { state: GameState },
    AnnouncementMsg { games: Vec<GameAnnouncement> },
    DiscoverMsg,
    JoinMsg {
        #[serde(rename = "type")]
        player_type: PlayerType,
        player_name: String,
        game_name: String,
        requested_role: NodeRole,
    },
    ErrorMsg { error_message: String },
    RoleChangeMsg {
        sender_role: Option<NodeRole>,
        receiver_role: Option<NodeRole>,
    },
}

#[derive(Debug, Serialize, Deserialize)]
struct GameMessage {
    msg_seq: i64,
    #[serde(default)]
    sender_id: Option<u64>,
    #[serde(default)]
    receiver_id: Option<u64>,
    #[serde(flatten)]
    msg_type: GameMessageType,
}

// fn master() {
//     // Obtain user input for game configuration
//     let config = get_game_config();
//
//     // Create a UDP socket for multicast messages
//     let multicast_socket: UdpSocket = create_multicast_socket().expect("Failed to create multicast socket");
//
//     // Create a UDP socket for regular communication
//     let port: u8 = 0; // todo input
//     let communication_socket: UdpSocket = create_communication_socket(port).expect("Failed to create communication socket");
//
//     // Start the game as the master node
//     let mut game_state = start_game(config.clone(), communication_socket);
//
//     // Start sending AnnouncementMsg with an interval of 1 second
//     send_announcement_messages(multicast_socket, game_state.clone());
//
//     // Create a thread to handle user input
//     thread::spawn(move || {
//         // Implement logic to handle user input and send corresponding messages
//         // For simplicity, we'll use a loop to simulate continuous user input
//         loop {
//             // Example: Send SteerMsg message to change the direction of the snake
//             let steer_msg = GameMessage {
//                 msg_seq: 1,
//                 sender_id: Some(1),
//                 receiver_id: None,
//                 msg_type: GameMessageType::SteerMsg { direction: Direction::UP },
//             };
//             send_game_message(&communication_socket, &steer_msg);
//
//             // Sleep for a short duration to simulate user input interval
//             thread::sleep(Duration::from_millis(100));
//         }
//     });
//
//     // Create a thread to handle incoming messages
//     thread::spawn(move || {
//         // Implement logic to receive and process incoming messages
//         loop {
//             // Example: Receive and process incoming messages
//             receive_and_process_messages(&communication_socket, &mut game_state);
//
//             // Sleep for a short duration to control the frequency of message processing
//             thread::sleep(Duration::from_millis(50));
//         }
//     });
//
//     // Keep the main thread alive
//     loop {
//         thread::sleep(Duration::from_secs(10));
//     }
// }

fn get_game_config() -> GameConfig {
    todo!()
}
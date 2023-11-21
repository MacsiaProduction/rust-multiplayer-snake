extern crate serde;

use snake::Direction;
use self::serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename = "NodeRole")]
pub enum NodeRole {
    NORMAL = 0,
    MASTER = 1,
    DEPUTY = 2,
    VIEWER = 3,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
enum PlayerType {
    HUMAN = 0,
    ROBOT = 1,
}

#[derive(Debug, Serialize, Deserialize)]
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

#[derive(Debug, Serialize, Deserialize)]
struct GameConfig {
    #[serde(default = "default_width")]
    width: u64,
    #[serde(default = "default_height")]
    height: u64,
    #[serde(default = "default_food_static")]
    food_static: u64,
    #[serde(default = "default_state_delay_ms")]
    state_delay_ms: u64,
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

#[derive(Debug, Serialize, Deserialize)]
struct GamePlayers {
    players: Vec<GamePlayer>,
}

#[derive(Debug, Serialize, Deserialize)]
struct Coord {
    #[serde(default)]
    x: u64,
    #[serde(default)]
    y: u64,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
enum SnakeState {
    ALIVE = 0,
    ZOMBIE = 1,
}

#[derive(Debug, Serialize, Deserialize)]
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

#[derive(Debug, Serialize, Deserialize)]
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
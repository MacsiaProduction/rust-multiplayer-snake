extern crate serde;
extern crate tokio;

use crate::snake::{Direction, Snake};
use self::serde::{Deserialize, Serialize};

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
pub(super) struct Coord {
    #[serde(default)]
    pub(crate) x: u64,
    #[serde(default)]
    pub(crate) y: u64,
}

impl Coord {
    pub(crate) fn new(x:u64, y:u64) -> Self {
        Coord {
            x,
            y,
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub(super) struct GameState {
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
    msg_seq: u64,
    #[serde(default)]
    sender_id: Option<u64>,
    #[serde(default)]
    receiver_id: Option<u64>,
    #[serde(flatten)]
    msg_type: GameMessageType,
}
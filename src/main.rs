extern crate piston_window;
extern crate rand;
extern crate serde;
extern crate tokio;

mod drawing;
mod game_state;
mod snake;

use std::collections::{HashMap, HashSet};
use std::net::{Ipv4Addr, SocketAddr};
use tokio::sync::Mutex;
use std::sync::{Arc};
use std::sync::atomic::Ordering;
use piston_window::*;
use piston_window::types::Color;
use tokio::net::UdpSocket;
use crate::connection::init_controller;
use self::serde::{Deserialize, Serialize};

use crate::drawing::*;
use crate::game_state::{GamePlayer, GamePlayers, GameState, PlayerType};
use crate::GameMessageType::AnnouncementMsg;
use crate::NodeRole::MASTER;
use crate::snake::Direction;

const BACK_COLOR: Color = [0.204, 0.286, 0.369, 1.0];

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
    500
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename = "NodeRole")]
pub enum NodeRole {
    NORMAL = 0,
    MASTER = 1,
    DEPUTY = 2,
    VIEWER = 3,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct GameAnnouncement {
    players: GamePlayers,
    config: GameConfig,
    #[serde(default = "default_can_join")]
    can_join: bool,
    game_name: String,
}

impl GameAnnouncement {
    fn new(config: GameConfig, name: String, id: u64, node_role: NodeRole) -> Self {
        GameAnnouncement {
            players: GamePlayers { players: vec!(GamePlayer::new(name.clone(), id, node_role))},
            config,
            can_join: true,
            game_name: format!("{}'s Game", name),
        }
    }

    fn new_with_ip(config: GameConfig, name: String, id: u64, node_role: NodeRole, ipv4addr: String, port: u16) -> Self {
        GameAnnouncement {
            players: GamePlayers { players: vec!(GamePlayer::new_with_ip(name.clone(), id, node_role, ipv4addr, port))},
            config,
            can_join: true,
            game_name: format!("{}'s Game", name),
        }
    }
}

fn default_can_join() -> bool {
    true
}

//todo numbers according to protobuf
#[derive(Debug, Serialize, Deserialize, Clone)]
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

#[tokio::main]
async fn main() {
    let name: String = "Macsia".to_string(); //todo get
    let config = GameConfig::default(); //todo get

    let local_addr1 = SocketAddr::new("127.0.0.1".parse().unwrap(), 9192);
    let multicast_socket: UdpSocket = UdpSocket::bind(local_addr1).await.expect("failed to create multicast socket");
    multicast_socket.join_multicast_v4(Ipv4Addr::new(239, 192, 0,4),Ipv4Addr::UNSPECIFIED).expect("failed to connect multicast group");
    multicast_socket.connect(SocketAddr::new("239.192.0.4".parse().unwrap(), 9192)).await.expect("failed to connect multicast group");

    //todo start screen with choice connect or create
    let local_addr2 = SocketAddr::new("127.0.0.1".parse().unwrap(), 0);
    let communication_socket =  Arc::new(Mutex::new(UdpSocket::bind(local_addr2).await.expect("failed to create communication socket")));
    let real_addr = communication_socket.lock().await.local_addr().unwrap().clone();
    let _selected: GameAnnouncement =
        GameAnnouncement::new_with_ip(
            config.clone(),
            name.clone(),
            1,
            MASTER,
            real_addr.ip().to_string(),
            real_addr.port()
        );

    communication_socket.lock().await.connect(real_addr).await.expect("failed to connect to master"); // loopback for master

    let window = init_window(&config);

    let game_state = Arc::new(Mutex::new(GameState::new(
        config.clone(),
        name.clone(),
        real_addr.ip().to_string(),
        real_addr.port()
    )));

    init_controller(window, communication_socket, game_state, multicast_socket).await;
}

fn init_window(config: &GameConfig) -> PistonWindow {
    let mut window_settings = WindowSettings::new("Rust Snake",
                                                  [to_gui_coord_u64(config.width) as u32, to_gui_coord_u64(config.height)as u32]).exit_on_esc(true);

    // Fix vsync extension error for linux
    window_settings.set_vsync(true);

    window_settings.build().unwrap()
}

mod connection {
    extern crate piston_window;
    extern crate rand;
    extern crate serde;
    extern crate tokio;
    use std::collections::{HashMap, HashSet};
    use std::net::SocketAddr;
    use tokio::sync::Mutex;
    use std::sync::{Arc};
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::sync::atomic::Ordering::Relaxed;
    use std::time::Duration;
    use piston_window::*;
    use rand::random;
    use tokio::net::UdpSocket;
    use tokio::time::interval;

    use crate::{BACK_COLOR, GameMessage, GameMessageType};
    use crate::game_state::{GamePlayer, GamePlayers, GameState};
    use crate::snake::Direction;

    //todo change when join the game
    static SENDER_ID: AtomicU64 = AtomicU64::new(0);
    static COUNTER: AtomicU64 = AtomicU64::new(1);

    pub(super) async fn init_controller(window: PistonWindow, socket: Arc<Mutex<UdpSocket>>, game_state: Arc<Mutex<GameState>>, multicast_socket: UdpSocket) {
        let awaiting_packages: Arc<Mutex<HashMap<u64, HashSet<u64>>>> = Arc::new(Mutex::new(HashMap::new()));
        awaiting_packages.lock().await.insert(SENDER_ID.load(Relaxed), HashSet::new());
        communication_controller(
            Arc::clone(&game_state),
            socket.clone(),
            multicast_socket,
            awaiting_packages.clone()
        ); //similar for master and not
        event_loop(
            window,
            socket.clone(),
            game_state.clone(),
            awaiting_packages.clone()
        ).await;
    }

    async fn event_loop(mut window: PistonWindow, socket: Arc<Mutex<UdpSocket>>, game_state: Arc<Mutex<GameState>>, awaiting_packages: Arc<Mutex<HashMap<u64, HashSet<u64>>>>) {
        while let Some(event) = window.next() {

            // Catch the events of the keyboard
            if let Some(Button::Keyboard(key)) = event.press_args() {
                tokio::spawn(key_handler(key, socket.clone(), awaiting_packages.clone()));
            }
            let state = game_state.lock().await.clone();
            // Draw all of them
            window.draw_2d(&event, |c, g, _| {
                clear(BACK_COLOR, g);
                state.draw(&c, g);
            });
        }
    }

    async fn key_handler(key: Key, communication_socket: Arc<Mutex<UdpSocket>>, awaiting_packages: Arc<Mutex<HashMap<u64, HashSet<u64>>>>) {
        let dir = match key {
            Key::Up => Some(Direction::Up),
            Key::W => Some(Direction::Up),

            Key::Down => Some(Direction::Down),
            Key::S => Some(Direction::Down),

            Key::Left => Some(Direction::Left),
            Key::A => Some(Direction::Left),

            Key::Right => Some(Direction::Right),
            Key::D => Some(Direction::Right),
            // Ignore other keys
            _ => return,
        };

        let steer_msg = GameMessageType::SteerMsg {
            direction: dir.unwrap()
        };

        send_game_message_to_master(
            communication_socket.clone(),
            SENDER_ID.load(Relaxed),
            None,
            steer_msg,
            awaiting_packages.clone()
        ).await;
    }

    pub(super) async fn send_game_message_to_master(socket: Arc<Mutex<UdpSocket>>, sender_id:u64, receiver: Option<u64>, game_message_type: GameMessageType, awaiting_packages: Arc<Mutex<HashMap<u64, HashSet<u64>>>>) {
        let message: GameMessage = GameMessage {
            msg_seq: COUNTER.fetch_and(1, Ordering::AcqRel), // todo idk what's this
            sender_id: Some(sender_id),
            receiver_id: receiver,
            msg_type: game_message_type,
        };
        // Serialize the GameMessage to a JSON string
        let json_message = serde_json::to_string(&message).expect("failed to serialize the GameMessage");

        // Send the JSON string through the UDP socket
        tokio::spawn(async move {
            let mut interval = interval(Duration::from_micros(10)); //todo really 0.1 * state_delay_ms
            for _ in 0..8 {
                socket.lock().await.send(json_message.as_bytes()).await.expect("failed to send game message");
                interval.tick().await;
                if awaiting_packages.lock().await.get_mut(&sender_id).expect("awaiting_packages for player not initialized").remove(&message.msg_seq) {
                    break;
                }
            }
        });
    }

    async fn send_to_all(socket: Arc<Mutex<UdpSocket>>, game_message_type: GameMessageType, game_players: GamePlayers, awaiting_packages: Arc<Mutex<HashMap<u64, HashSet<u64>>>>) {
        for player in game_players.players {
            send_game_message(
                socket.clone(),
                game_message_type.clone(),
                SENDER_ID.load(Relaxed),
                player.ip_address.expect(format!("missing ip_addr field from {} player", player.name.clone()).as_str()),
                player.port.expect(format!("missing port field from {} player", player.name.clone()).as_str()),
                awaiting_packages.clone()
            ).await;
        }
    }

    async fn send_game_message(socket: Arc<Mutex<UdpSocket>>, game_message_type: GameMessageType, sender_id :u64, ip: String, port: u16, awaiting_packages: Arc<Mutex<HashMap<u64, HashSet<u64>>>>) {
        let message: GameMessage = GameMessage {
            msg_seq: COUNTER.fetch_add(1, Relaxed),
            sender_id: Some(sender_id),
            receiver_id: None,
            msg_type: game_message_type,
        };
        // Serialize the GameMessage to a JSON string
        let json_message = serde_json::to_string(&message).expect("failed to serialize the GameMessage");

        // Send the JSON string through the UDP socket
        tokio::spawn(async move {
            let mut interval = interval(Duration::from_micros(10)); //todo really 0.1 * state_delay_ms
            for _ in 0..8 {
                socket.lock().await.send_to(json_message.as_bytes(), format!("{ip}:{port}")).await.expect("error sending game message");
                interval.tick().await;
                if awaiting_packages.lock().await.get_mut(&sender_id).expect("awaiting_packages for player not initialized").remove(&message.msg_seq) {
                    break;
                }
            }
        });
    }

    fn communication_controller(game_state: Arc<Mutex<GameState>>, communication_socket: Arc<Mutex<UdpSocket>>, multicast_socket: UdpSocket, awaiting_packages: Arc<Mutex<HashMap<u64, HashSet<u64>>>>) {
        let moves: Arc<Mutex<HashMap<u64, Direction>>> = Arc::new(Mutex::new(HashMap::new()));

        let _state_handle = tokio::spawn(game_state_translator(
            game_state.clone(),
            communication_socket.clone(),
            awaiting_packages.clone()
        ));
        let _announce_handle = tokio::spawn(announce_translator(
            multicast_socket,
            game_state.clone()
        ));
        let _game_turn_handle = tokio::spawn(game_turn_controller(
            game_state.clone(),
            communication_socket.clone(),
            moves.clone(),
            awaiting_packages.clone()
        ));
        let _request_handle = tokio::spawn(request_controller(
            game_state.clone(),
            communication_socket.clone(),
            moves.clone(),
            awaiting_packages.clone()
        ));
    }

    async fn request_controller(game_state: Arc<Mutex<GameState>>, communication_socket: Arc<Mutex<UdpSocket>>, moves:Arc<Mutex<HashMap<u64, Direction>>>, awaiting_packages: Arc<Mutex<HashMap<u64, HashSet<u64>>>>) {
        let mut buffer = vec![0; 2048];
        let mut interval = interval(Duration::from_micros(10));

        loop {
            // Try to receive a packet without blocking
            match communication_socket.lock().await.try_recv_from(&mut buffer) {
                Ok ((bytes,addr)) => {
                    let game_message :GameMessage = serde_json::from_slice(&buffer[..bytes]).expect("failed to deserialize GameMessage");
                    let sender_id = find_player_id_by_ip(game_state.clone(), addr).await;
                    match game_message.msg_type {
                        GameMessageType::PingMsg => {
                            // если мы ничего не отправляли в течении GameTurn нужно отправить его
                            //todo обновить живость игрока
                        }
                        GameMessageType::SteerMsg {direction} => {
                            //получаем новое направление от игрока
                            moves.lock().await.insert(sender_id, direction);
                        }
                        GameMessageType::AckMsg => {
                            //знаем что можно не пересылать сообщение с game_message.msg_seq
                            awaiting_packages.lock().await.get_mut(&sender_id).unwrap().insert(game_message.msg_seq.clone());
                        }
                        GameMessageType::StateMsg {state} => {
                            //cохраняем новое состояние
                            game_state.lock().await.clone_from(&state);
                        }
                        GameMessageType::AnnouncementMsg { .. } => {
                            //рисуем идущие игры
                            todo!("рисуем идущие игры");
                        }
                        GameMessageType::DiscoverMsg => {
                            //отправляем в ответ AnnouncementMsg
                            let my_game = game_state.lock().await.get_announcement();
                            //todo maybe somehow several games
                            send_game_message(
                                communication_socket.clone(),
                                GameMessageType::AnnouncementMsg { games: vec![my_game] },
                                SENDER_ID.load(Relaxed),
                                addr.ip().to_string(),
                                addr.port(),
                                awaiting_packages.clone(),
                            ).await;
                        }
                        GameMessageType::JoinMsg { player_type, player_name, game_name, requested_role } => {
                            //добавляем игрока в игру
                            //todo check game_name to be equal
                            assert_eq!(game_state.lock().await.get_announcement().game_name,game_name, "checks the game_name param in JoinMsg");
                            let player = GamePlayer {
                                name: player_name,
                                id: random(), //todo generate with id generator
                                ip_address: Some(addr.ip().to_string()),
                                port: Some(addr.port()),
                                role: requested_role,
                                player_type,
                                score: 0,
                            };
                            awaiting_packages.lock().await.insert(player.id.clone(), HashSet::new());
                            game_state.lock().await.players.players.push(player);
                        }
                        GameMessageType::ErrorMsg { error_message } => {
                            //отобразить его на экране, не блокируя работу программы
                            eprint!("{}", error_message);
                        }
                        GameMessageType::RoleChangeMsg { .. } => {
                            //cменить отправителя по умолчанию для сокета, назначить нового депути
                            todo!("смена роли");
                        }
                    }
                }
                Err(_) => {
                    interval.tick().await;
                }
            }
        }
    }

    async fn find_player_id_by_ip(game_state: Arc<Mutex<GameState>>, addr: SocketAddr) -> u64 {
        game_state.lock().await.players.players.clone().iter().find(|p|
            p.ip_address.clone().unwrap() == addr.ip().to_string() && p.port.unwrap() == addr.port()).expect("No player with such ip:port found").id
    }

    async fn game_turn_controller(game_state: Arc<Mutex<GameState>>, communication_socket: Arc<Mutex<UdpSocket>>, moves: Arc<Mutex<HashMap<u64, Direction>>>, awaiting_packages: Arc<Mutex<HashMap<u64, HashSet<u64>>>>) {
        let delay = Duration::from_millis(game_state.lock().await.config.state_delay_ms.clone());
        loop {
            {
                game_state.lock().await.update_snake(moves.clone().lock().await.clone());
            }
            let message = GameMessageType::StateMsg {
                state: game_state.lock().await.clone(),
            };
            send_to_all(
                communication_socket.clone(),
                message,
                game_state.lock().await.players.clone(),
                awaiting_packages.clone()
            ).await;
            tokio::time::sleep(delay).await;
        }
    }

    async fn announce_translator(multicast_socket: UdpSocket, game_state: Arc<Mutex<GameState>>) {
        loop {
            let message = GameMessageType::AnnouncementMsg {
                games: vec![game_state.lock().await.get_announcement()],
            };
            let json_message = serde_json::to_string(&message).expect("failed to serialize the GameMessage");
            multicast_socket.send(json_message.as_bytes()).await.expect("Failed to send multicast announcement");
            tokio::time::sleep(Duration::from_secs(1)).await;
        }
    }

    async fn game_state_translator(game_state: Arc<Mutex<GameState>>, communication_socket: Arc<Mutex<UdpSocket>>, awaiting_packages: Arc<Mutex<HashMap<u64, HashSet<u64>>>>) {
        let delay = Duration::from_millis(game_state.lock().await.config.state_delay_ms.clone());
        loop {
            let game_state = GameMessageType::StateMsg {
                state: game_state.lock().await.clone(),
            };

            send_game_message_to_master(
                communication_socket.clone(),
                SENDER_ID.load(Relaxed),
                None,
                game_state,
                awaiting_packages.clone()
            ).await;
            tokio::time::sleep(delay).await;
        }
    }
}

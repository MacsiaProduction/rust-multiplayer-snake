extern crate piston_window;
extern crate rand;
extern crate serde;
extern crate tokio;

mod drawing;
mod game_state;
mod snakes;
mod dto;

use std::net::{Ipv4Addr, SocketAddr};
use tokio::sync::Mutex;
use std::sync::Arc;
use piston_window::*;
use piston_window::types::Color;
use tokio::net::UdpSocket;
use crate::connection::{init_master, init_slave};
use self::serde::{Deserialize, Serialize};

use crate::drawing::*;
use crate::game_state::{GamePlayers, GameState, PlayerType};
use crate::snakes::Direction;

const BACK_COLOR: Color = [0.204, 0.286, 0.369, 1.0];

//todo protobuf generation of classes
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

impl From<dto::GameConfig> {

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

    let multicast_socket: UdpSocket = UdpSocket::bind("0.0.0.0:0").await
        .expect("failed to create multicast socket");
    multicast_socket.join_multicast_v4(Ipv4Addr::new(239, 192, 0,4),Ipv4Addr::UNSPECIFIED)
        .expect("failed to join multicast group");
    multicast_socket.connect(SocketAddr::new("239.192.0.4".parse().unwrap(), 9192)).await
        .expect("failed to connect to multicast group");

    let communication_socket =  Arc::new(Mutex::new(UdpSocket::bind(SocketAddr::new("127.0.0.1".parse().unwrap(), 0)).await
        .expect("failed to create communication socket")));
    let real_addr = communication_socket.lock().await.local_addr().unwrap().clone();

    //todo start screen with choice connect or create
    let create: bool = true;

    if create {
        let config = GameConfig::default(); //todo get

        communication_socket.lock().await.connect(real_addr).await.expect("failed to connect to master"); // loopback

        let window = init_window(&config);

        let game_state = Arc::new(Mutex::new(GameState::new(
            config.clone(),
            name.clone(),
            real_addr.ip().to_string(),
            real_addr.port()
        )));

        init_master(window, communication_socket, game_state, multicast_socket).await;
    } else {
        let mut buffer = vec![0; 2048];
        let (bytes, sender_addr) = multicast_socket.recv_from(&mut buffer).await.expect("failed to receive GameAnnouncement");
        let game_message : GameMessage = serde_json::from_slice(&buffer[..bytes]).expect("failed to deserialize GameMessage");
        let selected: GameAnnouncement;
        match game_message.msg_type {
            GameMessageType::AnnouncementMsg {games} => {
                selected = games.get(0).expect("No games found in AnnouncementMsg").clone();
            }
            _ => panic!("received not AnnouncementMsg from multicast socket")
        }

        //connect to master
        communication_socket.lock().await.connect(sender_addr).await.expect("failed to connect to master");

        let join_msg: GameMessage = GameMessage {
            msg_seq: 0,
            sender_id: None,
            receiver_id: None,
            msg_type: GameMessageType::JoinMsg {
                player_type: PlayerType::HUMAN,
                player_name: name,
                game_name: selected.game_name,
                requested_role: NodeRole::NORMAL,
            }
        };

        //sending joining message to master
        let json_message = serde_json::to_string(&join_msg).expect("failed to serialize the GameMessage");
        communication_socket.lock().await.send(json_message.as_bytes()).await.expect("failed to send game message");

        //receiveing Acknowledge message from master
        let bytes = communication_socket.lock().await.recv(&mut buffer).await.expect("failed to receive AckMsg for joining");
        let game_message : GameMessage = serde_json::from_slice(&buffer[..bytes]).expect("failed to deserialize GameMessage");

        let my_id:u64;
        let master_id:u64;
        match game_message.msg_type {
            GameMessageType::AckMsg => {
                my_id = game_message.receiver_id.expect("received AckMsg don't have sender_id");
                master_id = game_message.sender_id.expect("received AckMsg don't have master_id");
            }
            _ => panic!("received not AckMsg when joining to master")
        }
        let window = init_window(&selected.config);

        init_slave(window, communication_socket, multicast_socket, master_id, my_id).await;
    }
}

fn init_window(config: &GameConfig) -> PistonWindow {
    let mut window_settings = WindowSettings::new("Rust Snake",
                                                  [to_gui_coord_u64(config.width) as u32, to_gui_coord_u64(config.height)as u32]).exit_on_esc(true);

    // Fix vsync extension error for linux
    window_settings.set_vsync(true);

    window_settings.build().unwrap()
}

pub(crate) mod connection {
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

    use crate::{BACK_COLOR, GameMessage, GameMessageType, NodeRole};
    use crate::connection::send::{send_ack_message, send_game_message, send_game_message_to_master, send_to_all};
    use crate::game_state::{GamePlayer, GameState};
    use crate::snakes::Direction;

    //todo change when join the game
    static MY_ID: AtomicU64 = AtomicU64::new(0);
    static MASTER_ID: AtomicU64 = AtomicU64::new(0);
    static COUNTER: AtomicU64 = AtomicU64::new(1);

    mod send {
        use std::collections::{HashMap, HashSet};
        use std::sync::Arc;
        use std::sync::atomic::Ordering;
        use std::sync::atomic::Ordering::{Relaxed, SeqCst};
        use std::time::Duration;
        use tokio::net::UdpSocket;
        use tokio::sync::Mutex;
        use tokio::time::sleep;
        use crate::{GameMessage, GameMessageType};
        use crate::connection::{COUNTER, MASTER_ID, MY_ID};
        use crate::game_state::GamePlayers;

        pub(crate) async fn send_game_message_to_master(socket: Arc<Mutex<UdpSocket>>, sender_id:u64, receiver: Option<u64>, game_message_type: GameMessageType, awaiting_packages: Arc<Mutex<HashMap<u64, HashSet<u64>>>>) {
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
                let delay = Duration::from_micros(10); //todo 0.1 * state_delay_ms //todo really 0.1 * state_delay_ms
                for _ in 0..8 {
                    socket.lock().await.send(json_message.as_bytes()).await.expect("failed to send game message");
                    sleep(delay).await;
                    if awaiting_packages.lock().await.get_mut(&sender_id).expect("awaiting_packages for player not initialized").remove(&message.msg_seq) {
                        break;
                    }
                }
            });
        }

        pub(super) async fn send_to_all(socket: Arc<Mutex<UdpSocket>>, game_message_type: GameMessageType, game_players: GamePlayers, awaiting_packages: Arc<Mutex<HashMap<u64, HashSet<u64>>>>) {
            for player in game_players.players {
                send_game_message(
                    socket.clone(),
                    game_message_type.clone(),
                    MY_ID.load(Relaxed),
                    Some(player.id),
                    player.ip_address.expect(format!("missing ip_addr field from {} player", player.name.clone()).as_str()),
                    player.port.expect(format!("missing port field from {} player", player.name.clone()).as_str()),
                    awaiting_packages.clone()
                ).await;
            }
        }

        pub(super) async fn send_game_message(socket: Arc<Mutex<UdpSocket>>, game_message_type: GameMessageType, sender_id :u64, receiver_id: Option<u64>, ip: String, port: u16, awaiting_packages: Arc<Mutex<HashMap<u64, HashSet<u64>>>>) {
            let message: GameMessage = GameMessage {
                msg_seq: COUNTER.fetch_add(1, Relaxed),
                sender_id: Some(sender_id),
                receiver_id,
                msg_type: game_message_type,
            };
            // Serialize the GameMessage to a JSON string
            let json_message = serde_json::to_string(&message).expect("failed to serialize the GameMessage");

            // Send the JSON string through the UDP socket
            tokio::spawn(async move {
                let delay = Duration::from_micros(10); //todo 0.1 * state_delay_ms
                for _ in 0..8 {
                    socket.lock().await.send_to(json_message.as_bytes(), format!("{ip}:{port}")).await.expect("error sending game message");
                    sleep(delay).await;
                    if awaiting_packages.lock().await.get_mut(&sender_id).expect("awaiting_packages for player not initialized").remove(&message.msg_seq) {
                        break;
                    }
                }
            });
        }

        pub(super) async fn send_ack_message(socket: Arc<Mutex<UdpSocket>>, msg_seq: u64) {
            let message: GameMessage = GameMessage {
                msg_seq,
                sender_id: Some(MY_ID.load(SeqCst)),
                receiver_id: Some(MASTER_ID.load(SeqCst)),
                msg_type: GameMessageType::AckMsg {},
            };
            match serde_json::to_string(&message) {
                Ok(json_message) => if let Err(err) = socket.lock().await.send(json_message.as_bytes()).await {
                    println!("Failed to send game message: {}", err);
                },
                Err(err) => println!("Failed to serialize the GameMessage: {}", err),
            }
            println!("3");
        }
    }

    pub(super) async fn init_master(window: PistonWindow, socket: Arc<Mutex<UdpSocket>>, game_state: Arc<Mutex<GameState>>, multicast_socket: UdpSocket) {
        let my_id = game_state.lock().await.players.players.get(0).unwrap().id;
        MASTER_ID.store(my_id, Ordering::SeqCst);
        MY_ID.store(my_id, Ordering::SeqCst);

        let awaiting_packages: Arc<Mutex<HashMap<u64, HashSet<u64>>>> = Arc::new(Mutex::new(HashMap::from([(MY_ID.load(Relaxed), HashSet::new())])));

        master_communication_controller(
            Arc::clone(&game_state),
            socket.clone(),
            multicast_socket,
            awaiting_packages.clone()
        );

        event_loop(
            window,
            socket.clone(),
            game_state.clone(),
            awaiting_packages.clone()
        ).await;
    }

    pub(super) async fn init_slave(window: PistonWindow, socket: Arc<Mutex<UdpSocket>>, multicast_socket: UdpSocket, master_id:u64, slave_id:u64) {
        MASTER_ID.store(master_id, Ordering::SeqCst);
        MY_ID.store(slave_id, Ordering::SeqCst);

        let awaiting_packages: Arc<Mutex<HashMap<u64, HashSet<u64>>>> = Arc::new(Mutex::new(HashMap::from([(MY_ID.load(Relaxed), HashSet::new())])));

        let game_state = Arc::new(Mutex::default());

        tokio::spawn(slave_communication_controller(
            Arc::clone(&game_state),
            socket.clone(),
            multicast_socket,
            awaiting_packages.clone()
        ));

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
            MY_ID.load(Relaxed),
            Some(MASTER_ID.load(Relaxed)),
            steer_msg,
            awaiting_packages.clone()
        ).await;
    }

    fn master_communication_controller(game_state: Arc<Mutex<GameState>>, communication_socket: Arc<Mutex<UdpSocket>>, multicast_socket: UdpSocket, awaiting_packages: Arc<Mutex<HashMap<u64, HashSet<u64>>>>) {
        let moves: Arc<Mutex<HashMap<u64, Direction>>> = Arc::new(Mutex::new(HashMap::new()));

        let _request_controller_handle = tokio::spawn(request_controller(
            game_state.clone(),
            communication_socket.clone(),
            moves.clone(),
            awaiting_packages.clone()
        ));
        let _game_turn_controller_handle = tokio::spawn(game_turn_controller(
            game_state.clone(),
            communication_socket.clone(),
            moves.clone(),
            awaiting_packages.clone()
        ));
        let _state_translator_handle = tokio::spawn(game_state_translator(
            game_state.clone(),
            communication_socket.clone(),
            awaiting_packages.clone()
        ));
        let _announce_translator_handle = tokio::spawn(announce_translator(
            multicast_socket,
            game_state.clone()
        ));
    }

    async fn slave_communication_controller(game_state: Arc<Mutex<GameState>>, communication_socket: Arc<Mutex<UdpSocket>>, multicast_socket: UdpSocket, awaiting_packages: Arc<Mutex<HashMap<u64, HashSet<u64>>>>) {
        let moves: Arc<Mutex<HashMap<u64, Direction>>> = Arc::new(Mutex::new(HashMap::new()));

        let _game_turn_controller_handle = tokio::spawn(game_turn_controller(
            game_state.clone(),
            communication_socket.clone(),
            moves.clone(),
            awaiting_packages.clone()
        ));

        let _request_controller_handle = tokio::spawn(request_controller(
            game_state.clone(),
            communication_socket.clone(),
            moves.clone(),
            awaiting_packages.clone()
        ));

        let mut interval = interval(Duration::from_millis(game_state.lock().await.config.state_delay_ms.clone()));
        loop {
            if game_state.lock().await.players.players.get(MY_ID.load(Relaxed) as usize).unwrap().role.eq(&NodeRole::MASTER) {
                _request_controller_handle.abort();
                _game_turn_controller_handle.abort();
                master_communication_controller(game_state, communication_socket, multicast_socket, awaiting_packages);
                return;
            } else {
                interval.tick().await;
            }
        }
    }

    async fn request_controller(game_state: Arc<Mutex<GameState>>, communication_socket: Arc<Mutex<UdpSocket>>, moves:Arc<Mutex<HashMap<u64, Direction>>>, awaiting_packages: Arc<Mutex<HashMap<u64, HashSet<u64>>>>) {
        let mut buffer = vec![0; 2048];
        let mut interval = interval(Duration::from_micros(10));

        loop {
            // Try to receive a packet without blocking
            match communication_socket.lock().await.try_recv_from(&mut buffer) {
                Ok ((bytes,addr)) => {
                    let game_message :GameMessage = serde_json::from_slice(&buffer[..bytes]).expect("failed to deserialize GameMessage");
                    let sender = find_player_id_by_ip(game_state.clone(), addr).await;
                    match game_message.msg_type {
                        GameMessageType::PingMsg => {
                            // если мы ничего не отправляли в течении GameTurn нужно отправить его
                            //todo обновить живость игрока
                        }
                        GameMessageType::SteerMsg {direction} => {
                            //получаем новое направление от игрока
                            moves.lock().await.insert(sender.id, direction);
                        }
                        GameMessageType::AckMsg => {
                            //знаем что можно не пересылать сообщение с game_message.msg_seq
                            // assert_eq!(sender.id, game_message.sender_id.expect("protocol asserts GameMessage to have sender_id"), "only allow to receive AckMessages from Master");

                            awaiting_packages.lock().await.get_mut(&sender.id).unwrap().insert(game_message.msg_seq.clone());
                        }
                        GameMessageType::StateMsg {state} => {
                            //cохраняем новое состояние
                            if game_state.lock().await.state_order >= state.state_order {
                                continue;
                            }
                            game_state.lock().await.clone_from(&state);
                        }
                        GameMessageType::AnnouncementMsg { .. } => {
                            //ignored because we will not send discover while playing
                        }
                        GameMessageType::DiscoverMsg => {
                            //отправляем в ответ AnnouncementMsg
                            let my_game = game_state.lock().await.get_announcement();
                            //todo maybe somehow several games
                            send_game_message(
                                communication_socket.clone(),
                                GameMessageType::AnnouncementMsg { games: vec![my_game] },
                                MY_ID.load(Relaxed),
                                None,
                                addr.ip().to_string(),
                                addr.port(),
                                awaiting_packages.clone(),
                            ).await;
                        }
                        GameMessageType::JoinMsg { player_type, player_name, game_name, requested_role } => {
                            //добавляем игрока в игру
                            assert_eq!(game_state.lock().await.get_announcement().game_name, game_name, "checks the game_name param in JoinMsg");
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
                    println!("1");
                    send_ack_message(communication_socket.clone(), game_message.msg_seq.clone()).await;
                    println!("2");
                }
                Err(_) => {
                    interval.tick().await;
                }
            }
        }
    }

    async fn find_player_id_by_ip(game_state: Arc<Mutex<GameState>>, addr: SocketAddr) -> GamePlayer {
        game_state.lock().await.players.players.clone().iter().find(|p|
            p.ip_address.clone().unwrap() == addr.ip().to_string() && p.port.unwrap() == addr.port()).expect("No player with such ip:port found").clone()
    }

    async fn game_turn_controller(game_state: Arc<Mutex<GameState>>, communication_socket: Arc<Mutex<UdpSocket>>, moves: Arc<Mutex<HashMap<u64, Direction>>>, awaiting_packages: Arc<Mutex<HashMap<u64, HashSet<u64>>>>) {
        let delay = Duration::from_millis(game_state.lock().await.config.state_delay_ms.clone());
        let mut interval = interval(delay);
        loop {
            let mut moves_copy = moves.lock().await.clone();
            let mut state_copy = game_state.lock().await.clone();
            moves_copy.retain(|id, direction| {
               state_copy.steer_validate(*direction, *id)
            });
            state_copy.update_snake(moves_copy);
            let message = GameMessageType::StateMsg {
                state: state_copy.clone(),
            };
            send_to_all(
                communication_socket.clone(),
                message,
                state_copy.players,
                awaiting_packages.clone()
            ).await;
            interval.tick().await;
        }
    }

    async fn announce_translator(multicast_socket: UdpSocket, game_state: Arc<Mutex<GameState>>) {
        let mut interval = interval(Duration::from_secs(1));
        loop {
            let message = GameMessageType::AnnouncementMsg {
                games: vec![game_state.lock().await.get_announcement()],
            };
            let json_message = serde_json::to_string(&message).expect("failed to serialize the GameMessage");
            //todo разобраться почему нет сети
            multicast_socket.send(json_message.as_bytes()).await.expect("Failed to send multicast announcement");
            interval.tick().await;
        }
    }

    async fn game_state_translator(game_state: Arc<Mutex<GameState>>, communication_socket: Arc<Mutex<UdpSocket>>, awaiting_packages: Arc<Mutex<HashMap<u64, HashSet<u64>>>>) {
        let delay = Duration::from_millis(game_state.lock().await.config.state_delay_ms.clone());
        let mut interval = interval(delay);
        loop {
            let game_state = GameMessageType::StateMsg {
                state: game_state.lock().await.clone(),
            };

            send_game_message_to_master(
                communication_socket.clone(),
                MY_ID.load(Relaxed),
                None,
                game_state,
                awaiting_packages.clone()
            ).await;
            interval.tick().await;
        }
    }
}

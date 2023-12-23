extern crate piston_window;
extern crate tokio;

mod drawing;
mod game_state;
mod snakes;
mod dto;

use std::env;
use std::net::Ipv4Addr;
use tokio::sync::Mutex;
use std::sync::Arc;
use piston_window::*;
use piston_window::types::Color;
use protobuf::Message;
use tokio::net::UdpSocket;
use net2::UdpBuilder;
use net2::unix::UnixUdpBuilderExt;
use crate::connection::{init_master, init_slave};

use crate::drawing::*;
use crate::dto::*;

const BACK_COLOR: Color = [0.204, 0.286, 0.369, 1.0];

impl GameConfig {
    fn custom_default() -> Self {
        let mut config = GameConfig::default();
        config.set_height(20);
        config.set_width(20);
        config.set_food_static(5);
        config.set_state_delay_ms(300);
        config
    }
}

impl GamePlayer {
    fn custom_new(name: String, id:i32, ip: String, port:i32, role: NodeRole, player_type: PlayerType, score: i32) -> Self {
        let mut player:GamePlayer = GamePlayer::default();
        player.set_name(name);
        player.set_id(id);
        player.set_ip_address(ip);
        player.set_port(port);
        player.set_role(role);
        player.set_field_type(player_type);
        player.set_score(score);
        player
    }
}

impl GameMessage {
    fn custom_new(msq_seq:i64, sender_id:Option<i32>, receiver_id:Option<i32>, msg_type: GameMessage_oneof_Type) ->Self {
        let mut message:GameMessage = GameMessage::default();
        message.set_msg_seq(msq_seq);
        if let Some(sender) = sender_id {
            message.set_sender_id(sender);
        }
        if let Some(receiver) = receiver_id {
            message.set_receiver_id(receiver);
        }
        match msg_type {
            GameMessage_oneof_Type::ping(converted) => message.set_ping(converted),
            GameMessage_oneof_Type::steer(converted) => message.set_steer(converted),
            GameMessage_oneof_Type::ack(converted) => message.set_ack(converted),
            GameMessage_oneof_Type::state(converted) => message.set_state(converted),
            GameMessage_oneof_Type::announcement(converted) => message.set_announcement(converted),
            GameMessage_oneof_Type::join(converted) => message.set_join(converted),
            GameMessage_oneof_Type::error(converted) => message.set_error(converted),
            GameMessage_oneof_Type::role_change(converted) => message.set_role_change(converted),
            GameMessage_oneof_Type::discover(converted) => message.set_discover(converted),
        }
        message
    }

}

impl GameMessage_JoinMsg {
    fn custom_new(name: String, game_name: String, requested_role: NodeRole) -> Self {
        let mut message: GameMessage_JoinMsg = GameMessage_JoinMsg::default();
        message.set_player_name(name);
        message.set_game_name(game_name);
        message.set_player_type(PlayerType::HUMAN);
        message.set_requested_role(requested_role);
        message
    }
}

impl GameMessage_SteerMsg {
    fn custom_new(dir: Direction) -> Self {
        let mut msg: GameMessage_SteerMsg = GameMessage_SteerMsg::default();
        msg.set_direction(dir);
        msg
    }
}

impl GameMessage_StateMsg {
    fn custom_new(state: GameState) -> Self {
        let mut msg:GameMessage_StateMsg = GameMessage_StateMsg::default();
        msg.set_state(state);
        msg
    }
}

impl GameMessage_AnnouncementMsg {
    fn custom_new(game: GameAnnouncement) -> Self {
        let mut msg: GameMessage_AnnouncementMsg = GameMessage_AnnouncementMsg::default();
        msg.mut_games().push(game);
        msg
    }
}

#[tokio::main]
async fn main() {
    let args: Vec<String> = env::args().collect();

    let name: String;
    let create: bool;
    if args.len() == 1 {
        name = "Master".into();
        create = true;
    } else {
        name = args.get(1).unwrap().clone();
        create = args.get(2).is_some();
    }

    let socket = UdpBuilder::new_v4().unwrap().reuse_port(true).unwrap().bind("0.0.0.0:9192").unwrap();


    let multicast_socket: UdpSocket = UdpSocket::from_std(socket).unwrap();
    multicast_socket.join_multicast_v4(Ipv4Addr::new(239, 192, 0,4), Ipv4Addr::LOCALHOST).expect("failed to join multicast group");


    let communication_socket =  Arc::new(Mutex::new(UdpSocket::bind("127.0.0.1:0").await.expect("failed to create communication socket")));
    let real_addr = communication_socket.lock().await.local_addr().unwrap().clone();

    //todo start screen with choice connect or create

    println!("{}", real_addr);

    if create {
        let config = GameConfig::custom_default(); //todo get

        // communication_socket.lock().await.connect(real_addr).await.expect("failed to connect to master"); // loopback

        let window = init_window(&config);

        let game_state = Arc::new(Mutex::new(GameState::new_custom(
            name.clone(),
            real_addr.ip().to_string(),
            real_addr.port() as i32
        )));

        init_master(window, communication_socket, game_state, &config).await;
    } else {
        let mut buffer = vec![0; 2048];

        let (bytes, sender_addr) = multicast_socket.recv_from(&mut buffer).await.expect("failed to receive GameAnnouncement");


        let game_message : GameMessage = GameMessage::parse_from_bytes(&buffer[..bytes]).expect("failed to deserialize GameMessage");

        let selected: GameAnnouncement;
        if game_message.has_announcement() {
            let games = &game_message.get_announcement().get_games();
            println!("{:?}", games);
            selected = games.get(0).expect("No games found in AnnouncementMsg").clone();
        } else {
            panic!("received not AnnouncementMsg from multicast socket")
        }

        println!("1");

        let join_msg: GameMessage = GameMessage::custom_new(
            0,
            None,
            None,
            GameMessage_oneof_Type::join(GameMessage_JoinMsg::custom_new(name, selected.get_game_name().into(), NodeRole::NORMAL))
        );

        //sending joining message to master
        communication_socket.lock().await.send_to(&join_msg.write_to_bytes().expect("failed to serialize join message"), sender_addr).await.expect("failed to send game message");

        println!("sent join");

        let msg:GameMessage;

        //receiving Acknowledge message from master
        match communication_socket.lock().await.recv_from(&mut buffer).await {
            Ok((bytes, _)) => {
                msg = GameMessage::parse_from_bytes(&buffer[..bytes]).expect("failed to deserialize GameMessage");
                println!("{:?}", msg);
            },
            Err(e) => panic!("{}", e),
        }

        let my_id:i32;
        let master_id:i32;

        println!("{:?}", msg.Type);

        if msg.has_ack() {
            assert!(msg.clone().has_receiver_id(), "received AckMsg don't have sender_id");
            assert!(msg.clone().has_sender_id(), "received AckMsg don't have master_id");
            my_id = msg.get_receiver_id();
            master_id = msg.get_sender_id();
        } else {
            panic!("received not AckMsg when joining to master")
        }

        let mut game_state;
        loop {
            let ping_msg: GameMessage = GameMessage::custom_new(
                0,
                None,
                None,
                GameMessage_oneof_Type::ping(GameMessage_PingMsg::default()),
            );
            communication_socket.lock().await.send_to(&ping_msg.write_to_bytes().expect("failed to serialize join message"), sender_addr).await.expect("failed to send game message");

            let state:GameMessage;
            match communication_socket.lock().await.recv_from(&mut buffer).await {
                Ok((bytes, _)) => {
                    state = GameMessage::parse_from_bytes(&buffer[..bytes]).expect("failed to deserialize GameMessage");
                    println!("{:?}", state);
                },
                Err(e) => panic!("{}", e),
            }

            if state.has_state() {
                game_state = state.get_state().get_state().clone();
                if game_state.get_players().get_players().iter().find(|p| p.get_id() == my_id).is_some() {
                    break;
                }
            }
        }

        let window = init_window(&selected.get_config());

        init_slave(window, communication_socket, Arc::new(Mutex::new(game_state)), master_id, my_id, &selected.get_config()).await;
    }
}

fn init_window(config: &GameConfig) -> PistonWindow {
    let mut window_settings = WindowSettings::new("Rust Snake",
                                                  [to_gui_coord_f64(config.get_width())*1.5, to_gui_coord_f64(config.get_height())]);

    // Fix vsync extension error for linux
    window_settings.set_vsync(true);

    window_settings.exit_on_esc(true)
        .graphics_api(OpenGL::V3_2)
        .build()
        .unwrap()
}

pub(crate) mod connection {
    extern crate piston_window;
    extern crate rand;
    extern crate serde;
    extern crate tokio;

    use std::collections::{HashMap, HashSet};
    use std::net::SocketAddr;
    use tokio::sync::Mutex;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicI32, AtomicI64};
    use std::sync::atomic::Ordering::{Relaxed, SeqCst};
    use std::time::Duration;
    use piston_window::*;
    use protobuf::Message;
    use rand::random;
    use tokio::net::UdpSocket;
    use tokio::task::JoinHandle;
    use tokio::time::{interval, sleep};
    use crate::BACK_COLOR;

    use crate::connection::send::*;
    use crate::drawing::to_gui_coord_f64;
    use crate::dto::*;
    use crate::dto::NodeRole::{DEPUTY, MASTER, NORMAL, VIEWER};

    static MY_ID: AtomicI32 = AtomicI32::new(1);
    static MASTER_ID: AtomicI32 = AtomicI32::new(1);
    static COUNTER: AtomicI64 = AtomicI64::new(1); //zero was when we tried to connect

    mod send {
        use std::collections::{HashMap, HashSet};
        use std::sync::Arc;
        use std::sync::atomic::Ordering::Relaxed;
        use std::time::Duration;
        use protobuf::Message;
        use tokio::net::UdpSocket;
        use tokio::sync::Mutex;
        use tokio::time::sleep;
        use crate::connection::{COUNTER, MY_ID};
        use crate::dto::{GameMessage, GameMessage_AckMsg, GameMessage_oneof_Type, GamePlayer, GamePlayers};

        pub(super) async fn send_to_all(socket: Arc<Mutex<UdpSocket>>, game_message_type: GameMessage_oneof_Type, game_players: &GamePlayers, awaiting_packages: Arc<Mutex<HashMap<i32, HashSet<i64>>>>) {
            for player in game_players.get_players() {
                assert!(player.has_ip_address(), "{}", format!("missing ip_addr field from {} player", player.get_name()));
                assert!(player.has_port(), "{}", format!("missing port field from {} player", player.get_name()));
                send_game_message(
                    socket.clone(),
                    game_message_type.clone(),
                    Some(player.get_id()),
                    player.get_ip_address().into(),
                    player.get_port(),
                    awaiting_packages.clone()
                ).await;
            }
        }

        pub(super) async fn send_game_message(socket: Arc<Mutex<UdpSocket>>, game_message_type: GameMessage_oneof_Type, receiver_id: Option<i32>, ip: String, port: i32, awaiting_packages: Arc<Mutex<HashMap<i32, HashSet<i64>>>>) {
            let message = GameMessage::custom_new(
                COUNTER.fetch_add(1, Relaxed),
                Some(MY_ID.load(Relaxed)),
                receiver_id,
                game_message_type,
            );

            let bytes = message.write_to_bytes().expect("failed to serialize the GameMessage");

            // Send the JSON string through the UDP socket
            tokio::spawn(async move {
                let delay = Duration::from_micros(10); //todo 0.1 * state_delay_ms
                for _ in 0..8 {
                    let _ = socket.lock().await.try_send_to(&bytes, format!("{ip}:{port}").parse().unwrap());
                    sleep(delay).await;
                    if receiver_id.is_some() {
                        if awaiting_packages.lock().await.get_mut(&receiver_id.unwrap()).expect("awaiting_packages for player not initialized").remove(&message.get_msg_seq()) {
                            break;
                        }
                    }
                }
            });
        }

        pub(super) async fn send_ack_message(socket: Arc<Mutex<UdpSocket>>, msg_seq: i64, sender: GamePlayer) {
            let message: GameMessage = GameMessage::custom_new(
                msg_seq,
                Some(MY_ID.load(Relaxed)),
                Some(sender.get_id()),
                GameMessage_oneof_Type::ack(GameMessage_AckMsg::default()),
            );
            let bytes = message.write_to_bytes().expect("failed to serialize ack message");
            socket.lock().await.send_to(&*bytes, format!("{}:{}", sender.get_ip_address(), sender.get_port())).await.expect("failed to send ack message");
        }
    }

    fn init_awaiting_packages(players: &[GamePlayer]) -> Arc<Mutex<HashMap<i32, HashSet<i64>>>> {
        let mut tmp = HashMap::new();
        for player in players {
            tmp.insert(player.get_id(), HashSet::new());
        }
        return Arc::new(Mutex::new(tmp));
    }

    fn init_alive_players(players: &[GamePlayer]) -> Arc<Mutex<HashSet<i32>>> {
        let mut tmp = HashSet::new();
        for player in players {
            tmp.insert(player.get_id());
        }
        return Arc::new(Mutex::new(tmp));
    }

    pub(super) async fn init_master(window: PistonWindow, socket: Arc<Mutex<UdpSocket>>, game_state: Arc<Mutex<GameState>>, config: &GameConfig) {
        let my_id = game_state.lock().await.get_players().get_players().get(0).unwrap().get_id();
        MASTER_ID.store(my_id, SeqCst);
        MY_ID.store(my_id, SeqCst);

        let awaiting_packages = init_awaiting_packages(game_state.lock().await.get_players().get_players());

        master_communication_controller(
            game_state.clone(),
            socket.clone(),
            awaiting_packages.clone(),
            config.clone(),
        ).await;

        event_loop(
            window,
            socket.clone(),
            game_state.clone(),
            awaiting_packages.clone(),
            config
        ).await;
    }

    async fn master_killer(game_state: Arc<Mutex<GameState>>, config: GameConfig, handle: JoinHandle<()>) {
        let mut interval = interval(Duration::from_millis(config.get_state_delay_ms() as u64));
        loop {
            interval.tick().await;
            if game_state.lock().await.get_players().get_players().iter().find(|p| p.get_id() == MY_ID.load(SeqCst)).unwrap().get_role() == VIEWER {
                handle.abort();
                return;
            }
        }
    }

    pub(super) async fn init_slave(window: PistonWindow, socket: Arc<Mutex<UdpSocket>>, game_state: Arc<Mutex<GameState>>, master_id:i32, slave_id:i32, config: &GameConfig) {
        MASTER_ID.store(master_id, SeqCst);
        MY_ID.store(slave_id, SeqCst);

        let awaiting_packages = init_awaiting_packages(game_state.lock().await.get_players().get_players());

        tokio::spawn(slave_communication_controller(
            game_state.clone(),
            socket.clone(),
            awaiting_packages.clone(),
            config.clone()
        ));

        event_loop(
            window,
            socket.clone(),
            game_state.clone(),
            awaiting_packages.clone(),
            config
        ).await;
    }

    async fn event_loop(
        mut window: PistonWindow,
        socket: Arc<Mutex<UdpSocket>>,
        game_state: Arc<Mutex<GameState>>,
        awaiting_packages: Arc<Mutex<HashMap<i32, HashSet<i64>>>>,
        config: &GameConfig,
    ) {
        // Create a Glyphs object for rendering text
        let mut glyphs = window.load_font("/home/macsia/Downloads/RustRover-233.10527.212/jbr/lib/fonts/DroidSans.ttf").unwrap();

        while let Some(event) = window.next() {
            // Catch the events of the keyboard
            if let Some(Button::Keyboard(key)) = event.press_args() {
                tokio::spawn(key_handler(
                    key,
                    socket.clone(),
                    game_state.clone(),
                    awaiting_packages.clone(),
                ));
            }

            let state = game_state.lock().await.clone();

            // Draw all of them
            window.draw_2d(&event, |c, g, device| {
                clear(BACK_COLOR, g);

                // Draw the main game content
                state.draw(&c, g, config);

                // todo Draw the side panel with player information
                draw_side_panel(&c, g, config, &mut glyphs, &state);
                glyphs.factory.encoder.flush(device);
            });
        }
    }

    fn draw_side_panel(c: &Context, g: &mut G2d, config: &GameConfig, glyphs: &mut Glyphs, state: &GameState) {
        // Define the side panel dimensions
        let side_panel_width = to_gui_coord_f64(config.get_width()) / 2.0;
        let side_panel_height = to_gui_coord_f64(config.get_height());

        // Set the color for the side panel background
        let side_panel_color = [0.9, 0.9, 0.9, 1.0];

        // Draw the side panel background
        rectangle(
            side_panel_color,
            [to_gui_coord_f64(config.get_width()), 0.0, side_panel_width, side_panel_height],
            c.transform,
            g,
        );

        // Set the color for the text
        let text_color = [0.0, 0.0, 0.0, 1.0];

        // Set the font size
        let font_size = 15;

        // Draw player information on the side panel
        let mut y_position = 40.0; // Start with an initial Y position
        for player in state.get_players().get_players() {
            text(
                text_color,
                font_size,
                &format!("{}({:?}), score: {}", player.get_name(), player.get_role(), player.get_score()),
                glyphs,
                c.transform.trans(to_gui_coord_f64(config.get_width()) + 40.0, y_position),
                g,
            ).expect("failed to draw side panel");
            y_position += 30.0; // Adjust the spacing between player information
        }
    }


    async fn key_handler(key: Key, communication_socket: Arc<Mutex<UdpSocket>>, game_state: Arc<Mutex<GameState>>, awaiting_packages: Arc<Mutex<HashMap<i32, HashSet<i64>>>>) {
        let dir = match key {
            Key::Up => Some(Direction::UP),
            Key::W => Some(Direction::UP),

            Key::Down => Some(Direction::DOWN),
            Key::S => Some(Direction::DOWN),

            Key::Left => Some(Direction::LEFT),
            Key::A => Some(Direction::LEFT),

            Key::Right => Some(Direction::RIGHT),
            Key::D => Some(Direction::RIGHT),
            // Ignore other keys
            _ => return,
        };

        let steer_msg = GameMessage_SteerMsg::custom_new(dir.unwrap());

        if let Some(master) = game_state.lock().await.get_players().get_players().iter().find(|p| p.get_id()==MASTER_ID.load(Relaxed)).clone() {
            send_game_message(
                communication_socket.clone(),
                GameMessage_oneof_Type::steer(steer_msg),
                Some(master.get_id()),
                master.get_ip_address().into(),
                master.get_port(),
                awaiting_packages.clone(),
            ).await;
        }
    }

    async fn master_communication_controller(
        game_state: Arc<Mutex<GameState>>,
        communication_socket: Arc<Mutex<UdpSocket>>,
        awaiting_packages: Arc<Mutex<HashMap<i32, HashSet<i64>>>>,
        config: GameConfig,
    ) {
        let moves: Arc<Mutex<HashMap<i32, Direction>>> = Arc::new(Mutex::new(HashMap::new()));
        let players_alive = init_alive_players(game_state.lock().await.get_players().get_players());

        let _request_controller_handle = tokio::spawn(request_controller(
            game_state.clone(),
            communication_socket.clone(),
            moves.clone(),
            awaiting_packages.clone(),
            config.clone(),
            players_alive.clone()
        ));

        let _announce_translator_handle = tokio::spawn(announce_translator(
            communication_socket.clone(),
            game_state.clone(),
            config.clone()
        ));

        let _game_turn_controller_handle = tokio::spawn(game_turn_controller(
            game_state.clone(),
            communication_socket.clone(),
            moves.clone(),
            awaiting_packages.clone(),
            config.clone(),
            players_alive.clone()
        ));

        tokio::spawn(master_killer(game_state.clone(), config.clone(), _request_controller_handle));
        tokio::spawn(master_killer(game_state.clone(), config.clone(), _announce_translator_handle));
        tokio::spawn(master_killer(game_state.clone(), config.clone(), _game_turn_controller_handle));

    }

    async fn slave_communication_controller(
        game_state: Arc<Mutex<GameState>>,
        communication_socket: Arc<Mutex<UdpSocket>>,
        awaiting_packages: Arc<Mutex<HashMap<i32, HashSet<i64>>>>,
        config: GameConfig
    ) {
        let moves: Arc<Mutex<HashMap<i32, Direction>>> = Arc::new(Mutex::new(HashMap::new()));
        let alive_players = init_alive_players(game_state.lock().await.get_players().get_players());
        let request_controller_handle = tokio::spawn(request_controller(
            game_state.clone(),
            communication_socket.clone(),
            moves.clone(),
            awaiting_packages.clone(),
            config.clone(),
            alive_players.clone()
        ));

        let delay = Duration::from_secs_f32(config.get_state_delay_ms() as f32 / 1000f32 * 2.0);
        loop {
            alive_players.lock().await.clear();
            sleep(delay).await;
            // nothing received from master for interval
            if alive_players.lock().await.get(&MASTER_ID.load(SeqCst)).is_none() {
                println!("it's my star time");
                if game_state.lock().await.get_players().get_players().iter().find(|p| p.get_id() == MY_ID.load(SeqCst)).unwrap().get_role() == DEPUTY {
                    request_controller_handle.abort();

                    let mut game_state_copy = game_state.lock().await.clone();

                    game_state_copy.mut_players().mut_players().iter_mut().find(|p| p.get_id() == MASTER_ID.load(SeqCst)).unwrap().set_role(VIEWER);

                    game_state_copy.mut_players().mut_players().iter_mut().find(|p| p.get_id() == MY_ID.load(SeqCst)).unwrap().set_role(MASTER);

                    game_state.lock().await.clone_from(&game_state_copy);

                    MASTER_ID.store(MY_ID.load(SeqCst), SeqCst);

                    awaiting_packages.lock().await.insert(MASTER_ID.load(SeqCst), HashSet::new());

                    println!("now i master)");

                    master_communication_controller(game_state, communication_socket, awaiting_packages, config.clone()).await;
                    return;
                }
            }
        }
    }

    async fn request_controller(
        game_state: Arc<Mutex<GameState>>,
        communication_socket: Arc<Mutex<UdpSocket>>,
        moves:Arc<Mutex<HashMap<i32, Direction>>>,
        awaiting_packages: Arc<Mutex<HashMap<i32, HashSet<i64>>>>,
        config: GameConfig,
        players_alive: Arc<Mutex<HashSet<i32>>>,
    ) {
        let mut buffer = vec![0; 2048];
        let mut interval = interval(Duration::from_micros(10));

        loop {
            let result = communication_socket.lock().await.try_recv_from(&mut buffer);

            match result {
                Ok ((bytes,addr)) => {
                    let game_message = GameMessage::parse_from_bytes(&buffer[..bytes]).expect("failed to deserialize GameMessage");
                    let sender = find_player_id_by_ip(game_state.clone(), addr).await;

                    if !game_message.has_state() && !game_message.has_ack() {
                        println!("{:?}", &game_message.clone().Type.unwrap());
                    }

                    if !game_message.has_join() {
                        players_alive.lock().await.insert(sender.clone().expect("should be here...").get_id());
                    }

                    match game_message.Type.clone().unwrap() {
                        GameMessage_oneof_Type::ping(_converted) => {
                            // если мы ничего не отправляли в течении GameTurn нужно отправить его
                            send_ack_message(communication_socket.clone(), game_message.get_msg_seq(), sender.unwrap()).await;
                        },
                        GameMessage_oneof_Type::steer(converted) => {
                            //получаем новое направление от игрока
                            moves.lock().await.insert(sender.clone().unwrap().get_id(), converted.get_direction());
                            send_ack_message(communication_socket.clone(), game_message.get_msg_seq(), sender.unwrap()).await;
                        },
                        GameMessage_oneof_Type::ack(_converted) => {
                            //знаем что можно не пересылать сообщение с game_message.msg_seq

                            assert!(&game_message.clone().has_sender_id(), "protocol asserts GameMessage to have sender_id");
                            assert_eq!(sender.clone().unwrap().get_id(), game_message.clone().get_sender_id(), "only allow to receive AckMessages from Master");

                            awaiting_packages.lock().await.get_mut(&sender.unwrap().get_id()).unwrap().insert(game_message.get_msg_seq());
                        },
                        GameMessage_oneof_Type::state(converted) => {
                            //cохраняем новое состояние
                            if (game_state.lock().await.get_state_order() >= converted.get_state().get_state_order()) || (sender.clone().unwrap().get_id() != MASTER_ID.load(Relaxed)) {
                                continue;
                            }
                            game_state.lock().await.clone_from(&converted.get_state());
                            send_ack_message(communication_socket.clone(), game_message.get_msg_seq(), sender.unwrap()).await;
                        },
                        GameMessage_oneof_Type::announcement(_converted) => {
                            //ignored because we will not send discover while playing
                        },
                        GameMessage_oneof_Type::join(converted) => {
                            //добавляем игрока в игру
                            assert_eq!(game_state.lock().await.generate_announcement(config.clone()).get_game_name(), converted.get_game_name(), "checks the game_name param in JoinMsg");
                            let player = GamePlayer::custom_new(
                                converted.get_player_name().to_string(),
                                random(), //todo generate with id generator
                                addr.ip().to_string(),
                                addr.port() as i32,
                                converted.get_requested_role(),
                                converted.get_player_type(),
                                0,
                            );
                            awaiting_packages.lock().await.insert(player.get_id().clone(), HashSet::new());
                            players_alive.lock().await.insert(player.get_id().clone());
                            game_state.lock().await.mut_players().mut_players().push(player.clone());
                            game_state.lock().await.add_snake(player.get_id(), &config);
                            send_ack_message(communication_socket.clone(), game_message.get_msg_seq(), player).await;
                        },
                        GameMessage_oneof_Type::error(converted) => {
                            // отобразить его на экране, не блокируя работу программы
                            // todo new window
                            eprint!("{}", converted.get_error_message());
                            send_ack_message(communication_socket.clone(), game_message.get_msg_seq(), sender.unwrap()).await;
                        },
                        GameMessage_oneof_Type::role_change(converted) => {
                            //cменить отправителя по умолчанию для сокета...назначить нового депути
                            if MASTER_ID.load(SeqCst) == MY_ID.load(SeqCst) {
                                // если мы мастер то игнорим)
                                continue;
                            }
                            if converted.get_sender_role() == MASTER {
                                MASTER_ID.store(game_message.get_sender_id(), Relaxed);
                                awaiting_packages.lock().await.insert(MASTER_ID.load(SeqCst), HashSet::new());
                            }
                            send_ack_message(communication_socket.clone(), game_message.get_msg_seq(), sender.unwrap()).await;
                            // todo!("смена роли");
                        },
                        GameMessage_oneof_Type::discover(_converted) => {
                            //отправляем в ответ AnnouncementMsg
                            let my_game = game_state.lock().await.generate_announcement(config.clone());

                            send_game_message(
                                communication_socket.clone(),
                                GameMessage_oneof_Type::announcement(GameMessage_AnnouncementMsg::custom_new(my_game)),
                                None,
                                addr.ip().to_string(),
                                addr.port() as i32,
                                awaiting_packages.clone(),
                            ).await;
                        },
                    }
                }
                Err(_) => {
                    interval.tick().await;
                }
            }
        }
    }

    async fn find_player_id_by_ip(game_state: Arc<Mutex<GameState>>, addr: SocketAddr) -> Option<GamePlayer> {
        game_state.lock().await.get_players().get_players().iter().find(|p|
            p.get_ip_address().to_string() == addr.ip().to_string() && p.get_port() == addr.port() as i32).cloned()
    }


    //by master
    async fn game_turn_controller(
        game_state: Arc<Mutex<GameState>>,
        communication_socket: Arc<Mutex<UdpSocket>>,
        moves: Arc<Mutex<HashMap<i32, Direction>>>,
        awaiting_packages: Arc<Mutex<HashMap<i32, HashSet<i64>>>>,
        config: GameConfig,
        players_alive: Arc<Mutex<HashSet<i32>>>
    ) {
        let delay = Duration::from_millis(config.get_state_delay_ms() as u64);
        let mut interval = interval(delay);
        loop {
            let mut moves_copy = moves.lock().await.clone();
            let mut state_copy = game_state.lock().await.clone();
            moves_copy.retain(|id, direction| {
               state_copy.steer_validate(*direction, *id)
            });
            state_copy.update_snakes(&moves_copy, &config);

            for player in state_copy.mut_players().mut_players() {
                if !players_alive.lock().await.contains(&player.get_id()){
                    if player.get_role() == VIEWER || player.get_id() == MY_ID.load(SeqCst){
                        continue;
                    }
                    player.set_role(VIEWER);
                    println!("killed {}", player.get_id());
                    println!("{:?}", player);
                }
            }

            //trying to make new deputy
            if state_copy.get_players().get_players().len() > 1 {
                if state_copy.get_players().get_players().iter().find(|p| p.get_role() == DEPUTY).is_none() {
                    if let Some(player) = state_copy.mut_players().mut_players().iter_mut().find(|p| p.get_role() == NORMAL) {
                        player.set_role(DEPUTY);
                        let mut message = GameMessage_RoleChangeMsg::default();
                        message.set_receiver_role(DEPUTY);
                        message.set_sender_role(MASTER);
                        send_game_message(
                            communication_socket.clone(),
                            GameMessage_oneof_Type::role_change(message),
                            Some(player.get_id()),
                            player.get_ip_address().into(),
                            player.get_port(),
                            awaiting_packages.clone(),
                        ).await;
                    }
                }
            }

            players_alive.lock().await.clear();

            let message = GameMessage_StateMsg::custom_new(state_copy.clone());
            send_to_all(
                communication_socket.clone(),
                GameMessage_oneof_Type::state(message),
                state_copy.players.get_ref(),
                awaiting_packages.clone()
            ).await;
            interval.tick().await;
        }
    }

    async fn announce_translator(communication_socket: Arc<Mutex<UdpSocket>>, game_state: Arc<Mutex<GameState>>, config: GameConfig) {
        let mut interval = interval(Duration::from_secs(1));
        loop {

            let announcement = GameMessage_oneof_Type::announcement(GameMessage_AnnouncementMsg::custom_new(game_state.lock().await.generate_announcement(config.clone())));
            let message:GameMessage = GameMessage::custom_new(COUNTER.fetch_add(1,Relaxed), None, None, announcement);

            let bytes = message.write_to_bytes().expect("failed to serialize the GameMessage");
            communication_socket.lock().await.send_to(&bytes, "239.192.0.4:9192").await.expect("Failed to send multicast announcement");
            interval.tick().await;
        }
    }

}

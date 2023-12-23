extern crate serde;

use std::collections::HashMap;
use piston_window::types::Color;
use piston_window::*;

use crate::drawing::{draw_block, draw_rectangle};
use rand::{thread_rng, Rng};
use crate::dto::{Direction, GameAnnouncement, GameConfig, GamePlayer, GamePlayers, GameState, GameState_Coord, GameState_Snake, NodeRole};
use crate::dto::NodeRole::{MASTER, VIEWER};
use crate::dto::PlayerType::HUMAN;

const FOOD_COLOR: Color = [0.90, 0.49, 0.13, 1.0];
const BORDER_COLOR: Color = [0.741, 0.765, 0.78, 1.0];

impl GamePlayer {
    pub(crate) fn new_with_ip(name: String, id: i32, role: NodeRole, ipv4addr: String, port: i32) -> Self {
        let mut player = GamePlayer::default();
        player.set_score(0);
        player.set_id(id);
        player.set_name(name);
        player.set_role(role);
        player.set_ip_address(ipv4addr);
        player.set_port(port);
        player.set_field_type(HUMAN);
        player
    }
}

impl GameState {

    pub fn generate_announcement(&self, config: GameConfig) -> GameAnnouncement {
        let mut announcement = GameAnnouncement::default();
        announcement.set_can_join(true);
        if let Some(owner) = self.get_players().get_players().iter().find(|p| p.get_role() == MASTER) {
            announcement.set_game_name(owner.get_name().to_string()+" Game");
        } else {
            announcement.set_game_name("Unnamed Game".into());
        }
        announcement.set_config(config);
        announcement.set_players(self.get_players().clone());
        announcement
    }

    pub fn new_custom(name: String, ip: String, port: i32) -> GameState {
        let host_id = 1;
        let mut game_state : GameState = GameState::default();
        game_state.set_state_order(0);
        let mut players = GamePlayers::default();
        players.mut_players().push(GamePlayer::new_with_ip(name, host_id, MASTER, ip, port));
        game_state.set_players(players);
        game_state.mut_foods().push(GameState_Coord::new_custom(3,3));
        let snake = GameState_Snake::new_custom(2,2,host_id);
        game_state.snakes.push(snake);
        game_state
    }

    pub fn custom_default() -> GameState {
        GameState::new_custom("random".to_string(), "0.0.0.0".to_string(), 0)
    }

    pub fn steer_validate(&self, direction: Direction, sender_id: i32) -> bool{
        if self.get_players().get_players().iter()
            .find(|p| {p.get_id()==sender_id}).is_some_and(|p| {p.get_role() == VIEWER}) {
            return false;
        }

        if self.snakes.iter()
            .find(|s| {s.get_player_id() == sender_id})
            .is_some_and(|s| {s.get_head_direction().opposite() == direction}) {
            return false;
        }
        return true;
    }

    pub fn draw(&self, con: &Context, g: &mut G2d, config: &GameConfig) {
        self.snakes.iter().for_each(|s| s.draw(con, g, &config));

        self.foods.iter().for_each(|f| draw_block(FOOD_COLOR, f.get_x(), f.get_y(), con, g));

        // Draw the border
        draw_rectangle(BORDER_COLOR, 0, 0, config.get_width(), 1, con, g);
        draw_rectangle(BORDER_COLOR, 0,config.get_height() - 1, config.get_width(), 1, con, g);
        draw_rectangle(BORDER_COLOR, 0, 0, 1, config.get_height(), con, g);
        draw_rectangle(BORDER_COLOR, config.get_width() - 1, 0, 1, config.get_height(), con, g);
    }

    fn process_eating(&mut self, id: i32) -> bool {
        let head = self.get_snake(id).get_head_position().clone();

        if let Some(food_index) = self.get_foods().iter().position(|coord|
            &head == coord) {
            let player = self.mut_players().mut_players().iter_mut().find(|p| p.get_id() == id).unwrap();
            player.set_score(player.get_score()+1);
            self.foods.remove(food_index);
            return true;
        }
        return false;
    }

    fn check_if_the_snake_alive(&self, id: i32, width: i32, height: i32) -> bool {
        let head = self.get_snake(id).get_head_position();

        for s in self.get_snakes() {
            if id == s.get_player_id() {
                if s.is_overlap_except_head(head, width, height) {
                    return false;
                }
            } else if s.is_overlap(head, width, height) {
                return false;
            }
        };
        return true;
    }

    //todo optimize
    fn add_food(&mut self, config: &GameConfig) {
        let mut rng = thread_rng();

        loop {
            let new_x = rng.gen_range(1..(config.get_width() - 1));
            let new_y = rng.gen_range(1..(config.get_height() - 1));
            let new = GameState_Coord::new_custom(new_x, new_y);
            let condition = self.snakes.iter().any(|snake| {
                snake.is_overlap_except_head(&new, config.get_width(), config.get_height()) || snake.get_head_position() == &new
            });
            if !condition {
                self.foods.push(GameState_Coord::new_custom(new_x, new_y));
                break;
            }
        }
    }

    pub fn add_snake(&mut self, id: i32, config: &GameConfig) {
        let mut rng = thread_rng();
        //todo find free space
        let new_x = rng.gen_range(2..(config.get_width() - 2));
        let new_y = rng.gen_range(2..(config.get_height() - 2));
        let snake = GameState_Snake::new_custom(new_x, new_y, id);
        self.mut_snakes().push(snake);
    }

    pub fn update_snakes(&mut self, dirs: &HashMap<i32, Direction>, config: &GameConfig) {
        self.set_state_order(self.get_state_order()+1);

        for i in 0..self.get_snakes().len() {
            self.move_snake(self.get_snakes()[i].get_player_id(), dirs, config);
        }

        let mut players_to_kill : Vec<i32> = Vec::new();

        for snake in self.clone().get_snakes() {
            let id = snake.get_player_id();
            if !self.check_if_the_snake_alive(id, config.get_width(), config.get_height()) {
                players_to_kill.push(id);
            }
        }

        for id in players_to_kill {
            self.kill_player(id);
        }

        while self.foods.len() as i32 != config.get_food_static() {
            self.add_food(config);
        }
    }

    fn move_snake(&mut self, id: i32, dirs: &HashMap<i32, Direction>, config: &GameConfig) {

        self.get_snake_mut(id).move_forward_except_tail( dirs.get(&id).cloned(), config.get_width(), config.get_height());

        if !self.process_eating(id) {
            self.get_snake_mut(id).move_tail();
        }
    }

    pub fn kill_player(&mut self, player_id: i32) {
        if let Some(player) = self.mut_players().mut_players().iter_mut().find(|p| p.get_id() == player_id) {
            player.set_role(VIEWER);
            self.mut_snakes().retain(|s| s.get_player_id() != player_id);
        }
    }

    fn get_snake(&self, id: i32) -> &GameState_Snake {
        self.snakes.iter().find(|s| s.get_player_id() == id).unwrap()
    }

    fn get_snake_mut(&mut self, id: i32) -> &mut GameState_Snake {
        self.snakes.iter_mut().find(|s| s.get_player_id() == id).unwrap()
    }

}

extern crate serde;

use std::collections::HashMap;
use piston_window::types::Color;
use piston_window::*;

use crate::drawing::{draw_block, draw_rectangle};
use rand::{thread_rng, Rng};
use crate::{GameAnnouncement, GameConfig, NodeRole};
use crate::NodeRole::MASTER;
use self::serde::{Deserialize, Serialize};
use crate::snake::{Coord, Direction, Snake, SnakeState};

const FOOD_COLOR: Color = [0.90, 0.49, 0.13, 1.0];
const BORDER_COLOR: Color = [0.741, 0.765, 0.78, 1.0];
const GAMEOVER_COLOR: Color = [0.91, 0.30, 0.24, 0.5];

const MOVING_PERIOD: f64 = 0.1; // in second
const RESTART_TIME: f64 = 1.0; // in second

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "UPPERCASE")]
pub(super) enum PlayerType {
    HUMAN = 0,
    ROBOT = 1,
}


#[derive(Debug, Serialize, Deserialize, Clone)]
pub(super) struct GamePlayers {
    pub(crate) players: Vec<GamePlayer>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub(super) struct GamePlayer {
    pub(super) name: String,
    pub(super) id: u64,
    #[serde(default)]
    pub(super) ip_address: Option<String>,
    #[serde(default)]
    pub(super) port: Option<u16>,
    pub(crate) role: NodeRole,
    #[serde(default = "default_player_type")]
    #[serde(rename = "type")]
    pub(crate) player_type: PlayerType,
    pub(crate) score: u64,
}

impl GamePlayer {
    pub(crate) fn new_with_ip(name: String, id: u64, role: NodeRole, ipv4addr: String, port: u16) -> Self {
        GamePlayer {
            name,
            id,
            role,
            ip_address: Some(ipv4addr),
            port: Some(port),
            player_type: PlayerType::HUMAN,
            score: 0,
        }
    }
}

fn default_player_type() -> PlayerType {
    PlayerType::HUMAN
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub(super) struct GameState {
    state_order: u64,
    snakes: Vec<Snake>,
    foods: Vec<Coord>,
    pub(super) players: GamePlayers,
    #[serde(skip)]
    pub(crate) config: GameConfig,
}

impl GameState {

    pub fn get_announcement(&self) -> GameAnnouncement {
        let owner = self.players.players.iter().find(|p| p.role == MASTER).unwrap();
        GameAnnouncement {
            players: self.players.clone(),
            config: self.config.clone(),
            can_join: true,
            game_name: owner.name.to_string()+"Game",
        }
    }

    pub fn new(config: GameConfig, name: String, ip: String, port: u16) -> GameState {
        let host_id = 1;
        GameState {
            state_order: 0,
            snakes: vec!(Snake::new(2, 2, host_id)),
            foods: vec![],
            players: GamePlayers {
                players: vec!(GamePlayer::new_with_ip(name, host_id, MASTER, ip, port))
            },
            config
        }
    }

    pub fn steer_validate(&self, direction: Direction, sender_id: u64) -> bool{
        if self.players.players.iter()
            .find(|p| {p.id==sender_id}).is_some_and(|p| {p.role == NodeRole::VIEWER}) {
            return false;
        }

        if self.snakes.iter()
            .find(|s| {s.player_id == sender_id})
            .is_some_and(|s| {s.head_direction().opposite() == direction}) {
            return false;
        }
        return true;
    }

    pub fn draw(&self, con: &Context, g: &mut G2d) {
        self.snakes.iter().for_each(|s| {
            s.draw(con,g)
        });
        self.foods.iter().for_each(|f| {
            draw_block(FOOD_COLOR, f.x, f.y, con, g)
        });

        // Draw the border
        draw_rectangle(BORDER_COLOR, 0, 0, self.config.width, 1, con, g);
        draw_rectangle(BORDER_COLOR, 0, self.config.height - 1, self.config.width, 1, con, g);
        draw_rectangle(BORDER_COLOR, 0, 0, 1, self.config.height, con, g);
        draw_rectangle(BORDER_COLOR, self.config.width - 1, 0, 1, self.config.height, con, g);

        // if self.is_game_over {
        //      draw_rectangle(GAMEOVER_COLOR, 0, 0, self.config.width, self.config.height, con, g);
        // }
    }

    fn check_eating(&mut self) {
        for snake in &mut self.snakes {
            let (head_x, head_y) = snake.head_position();
            if let Some(food_index) = self.foods.iter().position(|f| f.x == head_x && f.y == head_y) {
                if let Some(player) = self.players.players.iter_mut().find(|p| p.id == snake.player_id) {
                    player.score += 1;
                }
                snake.restore_last_removed();
                self.foods.remove(food_index);
            }
        }
    }

    fn check_if_the_snake_touches_wall(&self, id: u64, dir: Option<Direction>) -> bool {
        let (x, y) = self.get_snake(id).next_head_position(dir);
        x == 0 || y == 0 || x == self.config.width - 1 || y == self.config.height - 1
    }

    fn check_if_the_snake_alive(&self, id: u64, dir: Option<Direction>) -> bool {
        let (next_x, next_y) = self.get_snake(id).next_head_position(dir);

        let mut flag: bool = true;
        self.snakes.iter().for_each(|s| {
            if s.is_overlap_except_tail(next_x, next_y) {
                flag = false;
            }
        });
        return flag;
    }

    fn add_food(&mut self) {
        let mut rng = thread_rng();

        //todo exhaustive search...
        loop {
            let new_x = rng.gen_range(1..(self.config.width - 1));
            let new_y = rng.gen_range(1..(self.config.height - 1));
            let condition = self.snakes.iter().any(|snake| {
                snake.points.iter().any(|point| point == &(new_x, new_y))
            });
            if !condition {
                self.foods.push(Coord::new(new_x, new_y));
                break;
            }
        }
    }

    pub fn update_snake(&mut self, users_dirs: HashMap<u64, Direction>) {
        // todo eat before collide
        for i in 0..self.snakes.len() {
            if let Some(dir) = users_dirs.get(&self.snakes[i].player_id) {
                self.move_snake(self.snakes[i].player_id, Some(*dir));
            } else {
                self.move_snake(self.snakes[i].player_id, None);
            }
        }
    }

    fn move_snake(&mut self, id: u64, dir: Option<Direction>) {
        let width = self.config.width.clone();
        let height = self.config.height.clone();
        if self.check_if_the_snake_touches_wall(id, dir) {
            self.get_snake_mut(id).go_through_wall(dir, width, height);
        } else if self.check_if_the_snake_alive(id, dir) {
            self.get_snake_mut(id).move_forward(dir);
            self.check_eating();
        } else {
            // player is dead
            self.kill_player(self.get_snake(id).player_id);
        }
    }

    fn kill_player(&mut self, player_id: u64) {
        if let Some(player) = self.players.players.iter_mut().find(|p| p.id == player_id) {
            player.role = NodeRole::VIEWER;
            self.snakes.retain(|s| s.player_id != player_id);
        }
    }

    fn zombify_player(&mut self, player_id: u64) {
        if let Some(snake) = self.snakes.iter_mut().find(|s| s.player_id == player_id) {
            snake.state = SnakeState::ZOMBIE;
            self.snakes.retain(|s| s.player_id != player_id);
        }
    }

    fn get_snake(&self, id: u64) -> &Snake {
        self.snakes.iter().find(|s| s.player_id == id).unwrap()
    }
    fn get_snake_mut(&mut self, id: u64) -> &mut Snake{
        self.snakes.iter_mut().find(|s| s.player_id == id).unwrap()
    }

}

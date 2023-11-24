use piston_window::types::Color;
use piston_window::*;

use crate::drawing::{draw_block, draw_rectangle};
use rand::{thread_rng, Rng};
use crate::connection::GameConfig;
use crate::snake::{Direction, Snake};

const FOOD_COLOR: Color = [0.90, 0.49, 0.13, 1.0];

//todo delete border
const BORDER_COLOR: Color = [0.741, 0.765, 0.78, 1.0];
const GAMEOVER_COLOR: Color = [0.91, 0.30, 0.24, 0.5];

const MOVING_PERIOD: f64 = 0.1; // in second
const RESTART_TIME: f64 = 1.0; // in second

pub struct Game {
    config: GameConfig,
    pub(super) snake: Snake,

    // Food
    food_exist: bool,
    food_x: u64,
    food_y: u64,

    // Game state
    is_game_over: bool,
    // Represents time from the previous moving
    waiting_time: f64,
}

impl Game {
    pub fn new(config: GameConfig) -> Game {
        Game {
            snake: Snake::new(2, 2),
            waiting_time: 0.0,
            food_exist: true,
            food_x: 5,
            food_y: 3,
            config,
            is_game_over: false,
        }
    }

    pub fn key_pressed(&mut self, key: Key) {
        if self.is_game_over {
            return;
        }

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

        if dir.unwrap() == self.snake.head_direction().opposite() {
            return;
        }

        // Check if the snake hits the border
        self.update_snake(dir);
    }

    pub fn draw(&self, con: &Context, g: &mut G2d) {
        self.snake.draw(con, g);

        // Draw the food
        if self.food_exist {
            draw_block(FOOD_COLOR, self.food_x, self.food_y, con, g);
        }

        // Draw the border
        draw_rectangle(BORDER_COLOR, 0, 0, self.config.width, 1, con, g);
        draw_rectangle(BORDER_COLOR, 0, self.config.height - 1, self.config.width, 1, con, g);
        draw_rectangle(BORDER_COLOR, 0, 0, 1, self.config.height, con, g);
        draw_rectangle(BORDER_COLOR, self.config.width - 1, 0, 1, self.config.height, con, g);

        // Draw a game-over rectangle
        if self.is_game_over {
            draw_rectangle(GAMEOVER_COLOR, 0, 0, self.config.width, self.config.height, con, g);
        }
    }
    pub fn update(&mut self, delta_time: f64) {
        self.waiting_time += delta_time;

        // Check if the food still exists
        if !self.food_exist {
            self.add_food();
        }

        // Move the snake
        if self.waiting_time > MOVING_PERIOD {
            self.update_snake(None);
        }
    }

    fn check_eating(&mut self) {
        let (head_x, head_y): (u64, u64) = self.snake.head_position();
        if self.food_exist && self.food_x == head_x && self.food_y == head_y {
            self.food_exist = false;
            self.snake.restore_last_removed();
        }
    }

    fn check_if_the_snake_touches_wall(&self, dir: Option<Direction>) -> bool {
        let (x, y) = self.snake.next_head_position(dir);
        x == 0 || y == 0 || x == self.config.width - 1 || y == self.config.height - 1
    }

    fn check_if_the_snake_alive(&self, dir: Option<Direction>) -> bool {
        let (next_x, next_y) = self.snake.next_head_position(dir);

        // Check if the snake hits itself
        if self.snake.is_overlap_except_tail(next_x, next_y) {
            return false;
        }
        // Check if the snake overlaps with the border
        next_x > 0 && next_y > 0 && next_x < self.config.width - 1 && next_y < self.config.height - 1
    }

    fn add_food(&mut self) {
        let mut rng = thread_rng();

        // Decide the position of the new food
        let mut new_x = rng.gen_range(1..(self.config.width - 1));
        let mut new_y = rng.gen_range(1..(self.config.height - 1));
        while self.snake.is_overlap_except_tail(new_x, new_y) {
            new_x = rng.gen_range(1..(self.config.width - 1));
            new_y = rng.gen_range(1..(self.config.height - 1));
        }

        // Add the new food
        self.food_x = new_x;
        self.food_y = new_y;
        self.food_exist = true;
    }

    fn update_snake(&mut self, dir: Option<Direction>) {
        if self.check_if_the_snake_touches_wall(dir) {
            self.snake.go_through_wall(dir, self.config.width, self.config.height);
        } else if self.check_if_the_snake_alive(dir) {
            self.snake.move_forward(dir);
            self.check_eating();
        } else {
            self.is_game_over = true;
        }
        self.waiting_time = 0.0;
    }
}

extern crate serde;

use piston_window::Context;
use piston_window::G2d;
use piston_window::types::Color;
use crate::connection::Coord;
use self::serde::{Deserialize, Serialize};

use crate::drawing::draw_block;

const SNAKE_COLOR: Color = [0.18, 0.80, 0.44, 1.0];

#[derive(Clone, Copy, PartialEq, Serialize, Deserialize, Debug)]
pub enum Direction {
    Up, Down, Left, Right
}

impl Direction {
    pub fn opposite(&self) -> Direction {
        match *self {
            Direction::Up => Direction::Down,
            Direction::Down => Direction::Up,
            Direction::Left => Direction::Right,
            Direction::Right => Direction::Left
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub(super) struct Snake {
    pub(super) player_id: u64,
    pub(super) points: Vec<Coord>,
    #[serde(default = "default_snake_state")]
    pub(super) state: SnakeState,
    head_direction: Direction,
    #[serde(skip)]
    last_removed: Option<Coord>,
}

fn default_snake_state() -> SnakeState {
    SnakeState::ALIVE
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "UPPERCASE")]
pub(super) enum SnakeState {
    ALIVE = 0,
    ZOMBIE = 1,
}

impl Snake {
    pub fn new(init_x: u64, init_y: u64, id: u64) -> Snake {
        let mut body: Vec<Coord> = Vec::new();
        body.push(Coord::new(init_x + 2, init_y));
        body.push(Coord::new(init_x + 1, init_y));

        Snake {
            player_id: id,
            head_direction: Direction::Right,
            points: body,
            state: default_snake_state(),
            last_removed: None,
        }
    }

    pub fn draw(&self, con: &Context, g: &mut G2d) {
        for coord in &self.points {
            draw_block(SNAKE_COLOR, coord.x, coord.y, con, g);
        }
    }

    pub fn go_through_wall(&mut self, dir: Option<Direction>, width: u64, height: u64) {
        // Retrieve the position of the head Coord
        let (last_x, last_y): (u64, u64) = self.head_position();

        match dir {
            Some(d) => self.head_direction = d,
            None => {}
        }

        // The moves
        let new_block = match self.head_direction {
            Direction::Up => Coord {
                x: last_x,
                y: height - 2
            },
            Direction::Down => Coord {
                x: last_x,
                y: 1
            },
            Direction::Left => Coord {
                x: width - 2,
                y: last_y
            },
            Direction::Right => Coord {
                x: 1,
                y: last_y
            }
        };

        self.points.insert(0,new_block);
        let removed_blk = self.points.pop().unwrap();
        self.last_removed = Some(removed_blk);
    }

    pub fn move_forward(&mut self, dir: Option<Direction>) {
        // Change moving direction
        match dir {
            Some(d) => self.head_direction = d,
            None => {}
        }

        // Retrieve the position of the head Coord
        let (last_x, last_y): (u64, u64) = self.head_position();

        // The snake moves
        let new_block = match self.head_direction {
            Direction::Up => Coord {
                x: last_x,
                y: last_y - 1
            },
            Direction::Down => Coord {
                x: last_x,
                y: last_y + 1
            },
            Direction::Left => Coord {
                x: last_x - 1,
                y: last_y
            },
            Direction::Right => Coord {
                x: last_x + 1,
                y: last_y
            }
        };
        self.points.insert(0,new_block);
        let removed_blk = self.points.pop().unwrap();
        self.last_removed = Some(removed_blk);
    }

    pub fn head_position(&self) -> (u64, u64) {
        let head_block = &self.points[0];
        (head_block.x, head_block.y)
    }

    pub fn head_direction(&self) -> Direction {
        self.head_direction
    }

    pub fn next_head_position(&self, dir: Option<Direction>) -> (u64, u64) {
        // Retrieve the position of the head Coord
        let (head_x, head_y): (u64, u64) = self.head_position();

        // Get moving direction
        let mut moving_dir = self.head_direction;
        match dir {
            Some(d) => moving_dir = d,
            None => {}
        }

        // The snake moves
        match moving_dir {
            Direction::Up => (head_x, head_y - 1),
            Direction::Down => (head_x, head_y + 1),
            Direction::Left => (head_x - 1, head_y),
            Direction::Right => (head_x + 1, head_y)
        }
    }

    pub fn restore_last_removed(&mut self) {
        let blk = self.last_removed.clone().unwrap();
        self.points.push(blk);
    }

    pub fn is_overlap_except_tail(&self, x: u64, y: u64) -> bool {
        let mut checked = 0;
        for coord in &self.points {
            if x == coord.x && y == coord.y {
                return true;
            }

            // For excluding the tail
            checked += 1;
            if checked == self.points.len() - 1 {
                break;
            }
        }
        return false;
    }
}

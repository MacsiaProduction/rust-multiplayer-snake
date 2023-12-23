extern crate serde;

use std::cmp::{max, min};
use piston_window::Context;
use piston_window::G2d;
use rand::{Rng, SeedableRng};

use crate::drawing::draw_block;
use crate::dto::{Direction, GameConfig, GameState_Coord, GameState_Snake};
use crate::dto::GameState_Snake_SnakeState::ALIVE;

impl GameState_Coord {
    pub fn new_custom(x:i32, y:i32) -> Self {
        let mut coord: GameState_Coord = GameState_Coord::default();
        coord.set_x(x);
        coord.set_y(y);
        coord
    }

    pub fn next(&self, delta: &GameState_Coord, width: i32, height: i32) -> GameState_Coord {
        let mut x = self.get_x() + delta.get_x();
        if x <= 0 {
            x = width-2;
        } else if x >= width-1 {
            x = 1
        }
        let mut y = self.get_y() + delta.get_y();
        if y <= 0 {
            y = height-2;
        } else if y >= height-1 {
            y = 1
        }
        GameState_Coord::new_custom(x, y)
    }

    pub fn next_with_dir(&self, dir: Direction, width: i32, height: i32) -> GameState_Coord {
        self.next(&dir.get_delta(), width, height)
    }

    pub fn check_next_for_jump(&self, delta: &GameState_Coord, width: i32, height: i32) -> bool {
        let x = self.get_x() + delta.get_x();
        if x <= 0 || x >= width-1 {
            return true;
        }
        let y = self.get_y() + delta.get_y();
        if y <= 0 || y >= height-1 {
            return true;
        }
        false
    }

    pub fn reverse(&self) -> GameState_Coord {
        GameState_Coord::new_custom(-self.get_x(), -self.get_y())
    }
}

impl From<GameState_Coord> for (i32, i32) {
    fn from(coord: GameState_Coord) -> Self {
        let x;
        if !coord.has_x() {
            x = 0;
        } else {
            x = coord.get_x();
        }

        let y;
        if !coord.has_y() {
            y = 0;
        } else {
            y = coord.get_y();
        }
        (x, y)
    }
}

impl Direction {
    pub fn opposite(&self) -> Direction {
        match *self {
            Direction::UP => Direction::DOWN,
            Direction::DOWN => Direction::UP,
            Direction::LEFT => Direction::RIGHT,
            Direction::RIGHT => Direction::LEFT
        }
    }

    pub fn get_delta(&self) -> GameState_Coord {
        match self {
            Direction::UP => GameState_Coord::new_custom(0, -1),
            Direction::DOWN => GameState_Coord::new_custom(0, 1),
            Direction::LEFT => GameState_Coord::new_custom(-1, 0),
            Direction::RIGHT => GameState_Coord::new_custom(1, 0),
        }
    }
}

impl GameState_Snake {
    pub fn new_custom(init_x: i32, init_y: i32, id: i32) -> GameState_Snake {
        let mut snake: GameState_Snake = GameState_Snake::default();

        snake.mut_points().push(GameState_Coord::new_custom(init_x, init_y));
        snake.mut_points().push(GameState_Coord::new_custom(-1, 0));

        snake.set_player_id(id);
        snake.set_head_direction(Direction::RIGHT);
        snake.set_state(ALIVE);
        snake
    }

    pub fn draw(&self, con: &Context, g: &mut G2d, game_config: &GameConfig) {
        let mut rng = rand::rngs::StdRng::seed_from_u64((self.get_player_id() as i64 + i32::MAX as i64) as u64);
        let color = generate_random_color(&mut rng);

        let mut cur = self.get_head_position().clone();
        draw_block(color, cur.get_x(), cur.get_y(), con, g);

        //todo оптимизировать особые клетки (после движения головы)
        //todo переход через границу нормальный
        for coord in self.get_points().iter().skip(1) {
            if cur.check_next_for_jump(&coord, game_config.get_width(), game_config.get_height()) {
                cur = cur.next(coord, game_config.get_width(), game_config.get_height());
                continue;
            }

            assert!((coord.get_x() == 0) || (coord.get_y() == 0), "impossible coord given");

            let range = min(cur.get_x(),cur.get_x() + coord.get_x())..=max(cur.get_x(),cur.get_x() + coord.get_x());

            for x in range {
                draw_block(color, x, cur.get_y(), con, g);
            }
            let range = min(cur.get_y(), cur.get_y() + coord.get_y())..=max(cur.get_y(), cur.get_y() + coord.get_y());

            for y in range {
                draw_block(color, cur.get_x(), y, con, g);
            }
            cur = cur.next(coord, game_config.get_width(), game_config.get_height());
        }
    }

    pub fn move_forward_except_tail(&mut self, dir: Option<Direction>, width:i32, height:i32) {
        // Change moving direction
        match dir {
            Some(d) => self.set_head_direction(d),
            None => {}
        }

        let delta = self.get_head_direction().get_delta();

        let head = self.points.remove(0);

        self.points.insert(0, delta.reverse());

        self.points.insert(0, head.next(&delta, width, height));
    }

    pub fn move_tail(&mut self) {
        let mut tail = self.get_points().get(self.get_points().len()-1).unwrap().clone();
        if tail.get_x()>0 {
            tail.set_x(tail.get_x()-1);
        }
        if tail.get_x()<0 {
            tail.set_x(tail.get_x()+1);
        }
        if tail.get_y()>0 {
            tail.set_y(tail.get_y()-1);
        }
        if tail.get_y()<0 {
            tail.set_y(tail.get_y()+1);
        }

        self.mut_points().pop();

        if tail.get_x()!=0 || tail.get_y()!=0 {
            self.mut_points().push(tail);
        }
    }

    pub fn next_head_position(&self, dir: Option<Direction>, width: i32, height:i32) -> GameState_Coord {

        let mut moving_dir = self.get_head_direction();

        match dir {
            Some(d) => moving_dir = d,
            None => {}
        }

        self.get_head_position().next_with_dir(moving_dir, width, height)
    }

    pub fn is_overlap_except_head(&self, coord: &GameState_Coord, width:i32, height:i32) -> bool {
        self.has_point_except_head_unoptimized(coord, width, height)
    }

    pub fn is_overlap(&self, coord: &GameState_Coord, width:i32, height:i32) -> bool {
        if self.get_head_position() == coord {
            return true;
        }
        return self.is_overlap_except_head(coord, width, height);
    }

    pub fn get_head_position(&self) -> &GameState_Coord {
        self.get_points().get(0).unwrap()
    }

    pub fn get_tail_position(&self, width: i32, height:i32) -> GameState_Coord {
        let mut cur = self.get_head_position().clone();
        for coord in self.get_points().iter().skip(1) {
            cur = cur.next(coord, width, height);
        }
        cur
    }

    fn _has_point(&self, point: &GameState_Coord, width: i32, height:i32) -> bool {
        let (point_x, point_y) = point.clone().into();
        let mut last = self.get_head_position().clone();

        for coord in self.get_points().iter().skip(1) {
            let (last_x, last_y) = last.clone().into();
            let x:bool;
            let y:bool;

            if coord.get_x() >= 0 {
                x = last_x<point_x && point_x<=(last_x+coord.get_x());
            } else {
                x = last_x>=point_x && point_x>(last_x+coord.get_x());
            }

            if coord.get_y() >= 0 {
                y = last_y<point_y && point_y<=(last_y+coord.get_y());
            } else {
                y = last_y>=point_y && point_y>(last_y+coord.get_y());
            }
            if x && y {
                return true;
            }
            last = last.next(coord, width, height);
        }
        return false;
    }

    pub fn has_point_except_head_unoptimized(&self, point: &GameState_Coord, width: i32, height:i32) -> bool {
        let mut body: Vec<GameState_Coord> = Vec::new();
        let mut cur = self.get_head_position().clone();

        //todo переход через границу нормальный
        for coord in self.get_points().iter().skip(1) {
            if cur.check_next_for_jump(&coord, width, height) {
                cur = cur.next(coord, width, height);
                continue;
                //TODO
            }

            assert!((coord.get_x() == 0) || (coord.get_y() == 0), "impossible coord given");

            let range = min(cur.get_x(), cur.get_x() + coord.get_x())..=max(cur.get_x(), cur.get_x() + coord.get_x());

            for x in range {
                body.push(GameState_Coord::new_custom(x, cur.get_y()));
            }
            let range = min(cur.get_y(), cur.get_y() + coord.get_y())..=max(cur.get_y(), cur.get_y() + coord.get_y());

            for y in range {
                body.push(GameState_Coord::new_custom(cur.get_x(), y));
            }
            cur = cur.next(coord, width, height);
        }

        let head = self.get_head_position();
        // Find the indices of elements in the first 4 positions that equal the head
        let indices_to_remove: Vec<usize> = body.iter().take(4).enumerate()
            .filter(|&(_, x)| x == head)
            .map(|(i, _)| i)
            .collect();

        // Remove elements at the found indices
        for &index in indices_to_remove.iter().rev() {
            body.remove(index);
        }

        return body.iter().any(|p| p == point)
    }
}

fn generate_random_color(rng: &mut impl Rng) -> [f32; 4] {
    // Generate random RGB values between 0.0 and 1.0
    let red = rng.gen_range(0.0..1.0);
    let green = rng.gen_range(0.0..1.0);
    let blue = rng.gen_range(0.0..1.0);

    // Set alpha to 1.0 (fully opaque)
    let alpha = 1.0;

    [red, green, blue, alpha]
}
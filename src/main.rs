extern crate piston_window;
extern crate rand;

mod snake;
mod game;
mod drawing;
mod connection;

use piston_window::*;
use piston_window::types::Color;

use game::Game;
use drawing::to_gui_coord_u64;
use crate::connection::GameConfig;

const BACK_COLOR: Color = [0.204, 0.286, 0.369, 1.0];

fn main() {
    let config = GameConfig::default(); // todo get from file

    // Prepare window settings
    let mut window_settings = WindowSettings::new("Rust Snake",
                                                  [to_gui_coord_u64(config.width) as u32, to_gui_coord_u64(config.height)as u32]).exit_on_esc(true);

    // Fix vsync extension error for linux
    window_settings.set_vsync(true);
    window_settings.get_fullscreen();

    // Create a window
    let mut window: PistonWindow = window_settings.build().unwrap();

    // Create a snake game
    let mut game = Game::new(config);

    // Event loop
    while let Some(event) = window.next() {

        // Catch the events of the keyboard
        if let Some(Button::Keyboard(key)) = event.press_args() {
            game.key_pressed(key);
        }

        // Draw all of them
        window.draw_2d(&event, |c, g, _| {
            clear(BACK_COLOR, g);
            game.draw(&c, g);
        });

        // Update the state of the game
        event.update(|arg| {
            game.update(arg.dt);
        });
    }
}

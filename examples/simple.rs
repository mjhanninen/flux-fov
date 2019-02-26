// simple.rs -- Flux FOV example
// Copyright (C) 2019 Matti Hänninen
//
// This program is free software: you can redistribute it and/or modify it
// under the terms of the GNU General Public License as published by the Free
// Software Foundation, either version 3 of the License, or (at your option)
// any later version.
//
// This program is distributed in the hope that it will be useful, but WITHOUT
// ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or
// FITNESS FOR A PARTICULAR PURPOSE.  See the GNU General Public License for
// more details.
//
// You should have received a copy of the GNU General Public License along
// with this program.  If not, see <http://www.gnu.org/licenses/>.

use flux_fov::{FluxField, Fov};
use rand::{self, Rng};
use tcod::{
    colors,
    console::{self, Console},
    input,
};

const WINDOW_SIZE: i32 = 49;

struct Map<T> {
    w: usize,
    h: usize,
    grid: Vec<Vec<T>>,
}

impl<T> Map<T>
where
    T: Clone,
{
    fn new(w: i32, h: i32, init: T) -> Self {
        let w = w as usize;
        let h = h as usize;
        let grid = vec![vec![init; h]; w];
        Map { w, h, grid }
    }
}

impl<T> Map<T> {
    fn at(&self, x: i32, y: i32) -> &T {
        &self.grid[x as usize][y as usize]
    }

    fn at_mut(&mut self, x: i32, y: i32) -> &mut T {
        &mut self.grid[x as usize][y as usize]
    }
}

#[derive(Clone, Default)]
struct Visibility {
    is_visible: bool,
    ray_output: f32,
}

fn main() {
    let mut root = console::Root::initializer()
        .title("Field of View study")
        .fullscreen(false)
        .size(WINDOW_SIZE, WINDOW_SIZE)
        .font("../assets/font.png", console::FontLayout::AsciiInRow)
        .init();

    let mut rng = rand::thread_rng();

    let mut map = Map::new(WINDOW_SIZE, WINDOW_SIZE, false);
    for _ in 0..20 {
        let x = rng.gen_range(0, WINDOW_SIZE);
        let y = rng.gen_range(0, WINDOW_SIZE);
        *map.at_mut(x, y) = true;
    }

    let flux_field = Box::new(FluxField::new(WINDOW_SIZE as usize));
    let mut fov = Fov::new(flux_field, WINDOW_SIZE as usize, Visibility::default());

    let mut player_x = rng.gen_range(0, WINDOW_SIZE);
    let mut player_y = rng.gen_range(0, WINDOW_SIZE);

    while !root.window_closed() {
        fov.update(|fov_x, fov_y, influxes| {
            let fov_dist_sq = fov_x * fov_x + fov_y * fov_y;
            if fov_dist_sq == 0 {
                Visibility {
                    is_visible: true,
                    ray_output: 1.0,
                }
            } else if fov_dist_sq < 400 {
                let map_x = player_x + fov_x;
                let map_y = player_y + fov_y;
                if 0 <= map_x && map_x < map.w as i32 && 0 <= map_y && map_y < map.h as i32 {
                    let ray_input = influxes.iter().map(|f| f.weight * f.value.ray_output).sum();
                    Visibility {
                        is_visible: ray_input > 0.75,
                        ray_output: if *map.at(map_x, map_y) {
                            0.0
                        } else {
                            ray_input
                        },
                    }
                } else {
                    Visibility {
                        is_visible: false,
                        ray_output: 0.0,
                    }
                }
            } else {
                Visibility {
                    is_visible: false,
                    ray_output: 0.0,
                }
            }
        });

        root.set_default_background(colors::BLACK);
        root.clear();
        for y in 0..WINDOW_SIZE {
            for x in 0..WINDOW_SIZE {
                let is_block = *map.at(x, y);
                let dx = x - player_x;
                let dy = y - player_y;
                let fg = if fov.at(dx, dy).is_visible {
                    if is_block {
                        colors::PURPLE
                    } else {
                        colors::YELLOW
                    }
                } else {
                    colors::DARK_BLUE
                };
                let bg = colors::BLACK;
                let glyph = if is_block { '#' } else { '.' };
                root.put_char_ex(x, y, glyph, fg, bg);
            }
        }
        root.set_default_foreground(colors::WHITE);
        root.put_char(player_x, player_y, '@', console::BackgroundFlag::None);

        root.flush();

        match root.wait_for_keypress(false) {
            input::Key {
                code: input::KeyCode::Escape,
                ..
            } => break,
            input::Key {
                code: input::KeyCode::Char,
                printable: 'k',
                ..
            } => {
                if player_y > 0 {
                    player_y -= 1;
                }
            }
            input::Key {
                code: input::KeyCode::Char,
                printable: 'h',
                ..
            } => {
                if player_x > 0 {
                    player_x -= 1;
                }
            }
            input::Key {
                code: input::KeyCode::Char,
                printable: 'j',
                ..
            } => {
                if player_y < WINDOW_SIZE - 1 {
                    player_y += 1;
                }
            }
            input::Key {
                code: input::KeyCode::Char,
                printable: 'l',
                ..
            } => {
                if player_x < WINDOW_SIZE - 1 {
                    player_x += 1;
                }
            }
            _ => (),
        }
    }
}

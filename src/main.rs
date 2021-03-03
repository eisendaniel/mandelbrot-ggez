use std::usize;

use ggez::{
    conf,
    event::{self, MouseButton},
    graphics::{self, Image},
    input, Context, ContextBuilder, GameResult,
};
use num::Complex;
use palette::rgb::Srgb;
use rayon::prelude::*;
/// Try to determine if `c` is in the Mandelbrot set, using at most `limit`
/// iterations to decide.
///
/// If `c` is not a member, return `Some(i)`, where `i` is the number of
/// iterations it took for `c` to leave the circle of radius two centered on the
/// origin. If `c` seems to be a member (more precisely, if we reached the
/// iteration limit without being able to prove that `c` is not a member),
/// return `None`.
fn escape_time(c: Complex<f64>, limit: u32) -> Option<u32> {
    let mut z = Complex { re: 0.0, im: 0.0 };
    for i in 0..limit {
        z = z * z + c;
        if z.norm_sqr() > 4.0 {
            return Some(i);
        }
    }
    None
}

/// Given the row and column of a pixel in the output image, return the
/// corresponding point on the complex plane.
///
/// `bounds` is a pair giving the width and height of the image in pixels.
/// `pixel` is a (column, row) pair indicating a particular pixel in that image.
/// The `upper_left` and `lower_right` parameters are points on the complex
/// plane designating the area our image covers.
fn pixel_to_point(
    bounds: (usize, usize),
    pixel: (usize, usize),
    upper_left: Complex<f64>,
    lower_right: Complex<f64>,
) -> Complex<f64> {
    let (width, height) = (
        lower_right.re - upper_left.re,
        upper_left.im - lower_right.im,
    );
    Complex {
        re: upper_left.re + pixel.0 as f64 * width / bounds.0 as f64,
        im: upper_left.im - pixel.1 as f64 * height / bounds.1 as f64, // Why subtraction here? pixel.1 increases as we go down,
                                                                       // but the imaginary component increases as we go up.
    }
}

/// Render a rectangle of the Mandelbrot set into a buffer of pixels.
///
/// The `bounds` argument gives the width and height of the buffer `pixels`,
/// which holds one grayscale pixel per byte. The `upper_left` and `lower_right`
/// arguments specify points on the complex plane corresponding to the upper-
/// left and lower-right corners of the pixel buffer.
fn render(
    pixels: &mut [u8],
    bounds: (usize, usize),
    upper_left: Complex<f64>,
    lower_right: Complex<f64>,
) {
    for row in 0..bounds.1 {
        for column in 0..bounds.0 {
            let point = pixel_to_point(bounds, (column, row), upper_left, lower_right);
            let val = match escape_time(point, 255) {
                None => 0,
                Some(count) => 255 - count as u8,
            };
            let is_c = true;
            if is_c {
                let col = Srgb::from(palette::Hsv::new(
                    360. * (val as f64 / 255.),
                    1.,
                    1. * (if val == 0 { 0. } else { 1. }),
                ));

                pixels[4 * (row * bounds.0 + column)] = (col.red * 255.) as u8;
                pixels[4 * (row * bounds.0 + column) + 1] = (col.green * 255.) as u8;
                pixels[4 * (row * bounds.0 + column) + 2] = (col.blue * 255.) as u8;
                pixels[4 * (row * bounds.0 + column) + 3] = 255;
            } else {
                pixels[4 * (row * bounds.0 + column)] = val;
                pixels[4 * (row * bounds.0 + column) + 1] = val;
                pixels[4 * (row * bounds.0 + column) + 2] = val;
                pixels[4 * (row * bounds.0 + column) + 3] = 255;
            }
        }
    }
}

fn draw_image(
    ctx: &mut Context,
    bounds: (usize, usize),
    upper_left: Complex<f64>,
    lower_right: Complex<f64>,
) -> graphics::Image {
    let mut pixels = vec![0; 4 * bounds.0 * bounds.1];

    // Scope of slicing up `pixels` into horizontal bands.
    {
        let bands: Vec<(usize, &mut [u8])> = pixels.chunks_mut(4 * bounds.0).enumerate().collect();

        bands.into_par_iter().for_each(|(i, band)| {
            let top = i;
            let band_bounds = (bounds.0, 1);
            let band_upper_left = pixel_to_point(bounds, (0, top), upper_left, lower_right);
            let band_lower_right =
                pixel_to_point(bounds, (bounds.0, top + 1), upper_left, lower_right);
            render(band, band_bounds, band_upper_left, band_lower_right);
        });
    }

    Image::from_rgba8(ctx, bounds.0 as u16, bounds.0 as u16, &pixels).unwrap()
}

fn main() {
    let win_size: (usize, usize) = (1024, 1024);

    let (mut ctx, mut events_loop) = ContextBuilder::new("Mandlebrot", "Daniel Eisen")
        .window_mode(conf::WindowMode::default().dimensions(win_size.0 as f32, win_size.1 as f32))
        .window_setup(conf::WindowSetup::default().samples(conf::NumSamples::Eight))
        .build()
        .expect("Failed to create context");

    let mut state = State::new(&mut ctx, win_size);

    match event::run(&mut ctx, &mut events_loop, &mut state) {
        Ok(_) => println!("Exited Cleanly "),
        Err(e) => println!("Error: {}", e),
    }
}

struct State {
    bounds: (usize, usize),
    upper_left: Complex<f64>,
    lower_right: Complex<f64>,
    texture: Image,
    mouse_pressed: bool,
    mouse_released: bool,
}

impl State {
    pub fn new(ctx: &mut Context, init_bounds: (usize, usize)) -> State {
        State {
            bounds: init_bounds,
            upper_left: Complex::new(-3., 2.),
            lower_right: Complex::new(1., -2.),
            texture: draw_image(
                ctx,
                init_bounds,
                Complex::new(-3., 2.),
                Complex::new(1., -2.),
            ),
            mouse_pressed: false,
            mouse_released: false,
        }
    }
}

impl event::EventHandler for State {
    fn update(&mut self, ctx: &mut Context) -> GameResult {
        if input::keyboard::is_key_pressed(ctx, input::keyboard::KeyCode::Space) {
            self.upper_left = Complex::new(-3., 2.);
            self.lower_right = Complex::new(1., -2.);
            self.texture = draw_image(ctx, self.bounds, self.upper_left, self.lower_right);
        }
        if self.mouse_released {
            self.texture = draw_image(ctx, self.bounds, self.upper_left, self.lower_right);
            self.mouse_released = false;
        }
        Ok(())
    }
    fn draw(&mut self, ctx: &mut Context) -> GameResult {
        graphics::clear(ctx, graphics::BLACK);
        graphics::draw(ctx, &self.texture, graphics::DrawParam::new())?;

        if self.mouse_pressed {
            let cursor = input::mouse::position(ctx);
            let rect = graphics::Mesh::new_rectangle(
                ctx,
                graphics::DrawMode::fill(),
                graphics::Rect::new(cursor.x - 100., cursor.y - 100., 200., 200.),
                [1., 1., 1., 0.2].into(),
            )?;
            graphics::draw(ctx, &rect, graphics::DrawParam::new())?;
        }

        graphics::present(ctx)
    }

    fn mouse_button_down_event(
        &mut self,
        _ctx: &mut Context,
        button: MouseButton,
        _x: f32,
        _y: f32,
    ) {
        if button == MouseButton::Left {
            self.mouse_pressed = true;
        }
    }

    fn mouse_button_up_event(&mut self, _ctx: &mut Context, button: MouseButton, x: f32, y: f32) {
        self.mouse_released = true;
        self.mouse_pressed = false;

        if button == MouseButton::Left {
            let new_u_l = pixel_to_point(
                self.bounds,
                ((x - 100.) as usize, (y - 100.) as usize),
                self.upper_left,
                self.lower_right,
            );
            let new_l_r = pixel_to_point(
                self.bounds,
                ((x + 100.) as usize, (y + 100.) as usize),
                self.upper_left,
                self.lower_right,
            );
            self.upper_left = new_u_l;
            self.lower_right = new_l_r;
        } else if button == MouseButton::Right {
            let diag = self.upper_left - self.lower_right;
            self.upper_left += diag;
            self.lower_right -= diag;
        }
    }
}

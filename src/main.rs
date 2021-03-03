use ggez::{
    conf, event,
    graphics::{self, Image},
    input::keyboard,
    nalgebra as na, Context, ContextBuilder, GameResult,
};
use image::{png::PngEncoder, ColorType, ImageError};
use num::Complex;
use rayon::prelude::*;
use std::fs::File;
use std::io::Write;
use std::str::FromStr;

/// Parse the string `s` as a coordinate pair, like `"400x600"` or `"1.0,0.5"`.
///
/// Specifically, `s` should have the form <left><sep><right>, where <sep> is
/// the character given by the `separator` argument, and <left> and <right> are both
/// strings that can be parsed by `T::from_str`.
///
/// If `s` has the proper form, return `Some<(x, y)>`. If it doesn't parse
/// correctly, return `None`.
fn parse_pair<T: FromStr>(s: &str, separator: char) -> Option<(T, T)> {
    match s.find(separator) {
        None => None,
        Some(index) => match (T::from_str(&s[..index]), T::from_str(&s[index + 1..])) {
            (Ok(l), Ok(r)) => Some((l, r)),
            _ => None,
        },
    }
}

/// Parse a pair of floating-point numbers separated by a comma as a complex
/// number.
fn parse_complex(s: &str) -> Option<Complex<f64>> {
    match parse_pair(s, ',') {
        Some((re, im)) => Some(Complex { re, im }),
        None => None,
    }
}

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
            let alpha = match escape_time(point, 255) {
                None => 255,
                Some(count) => count as u8,
            };
            pixels[4 * (row * bounds.0 + column)] = 255;
            pixels[(4 * (row * bounds.0 + column)) + 1] = 255;
            pixels[(4 * (row * bounds.0 + column)) + 2] = 255;
            pixels[(4 * (row * bounds.0 + column)) + 3] = alpha;
        }
    }
}

/// Write the buffer `pixels`, whose dimensions are given by `bounds`, to the
/// file named `filename`.
fn write_image(filename: &str, pixels: &[u8], bounds: (usize, usize)) -> Result<(), ImageError> {
    let output = File::create(filename)?;

    let encoder = PngEncoder::new(output);
    encoder.encode(&pixels, bounds.0 as u32, bounds.1 as u32, ColorType::Rgba8)?;

    Ok(())
}

fn main() {
    let args: Vec<String> = std::env::args().collect();

    if args.len() != 5 {
        match writeln!(
            std::io::stderr(),
            "Usage: mandelbrot FILE PIXELS UPPERLEFT LOWERRIGHT"
        ) {
            Ok(a) => a,
            _ => unreachable!(),
        };
        match writeln!(
            std::io::stderr(),
            "Example: {} mandel.png 1000x750 -1.20,0.35 -1,0.20",
            args[0]
        ) {
            Ok(a) => a,
            _ => unreachable!(),
        };
        std::process::exit(1);
    }

    let bounds = parse_pair(&args[2], 'x').expect("error parsing image dimensions");
    let upper_left = parse_complex(&args[3]).expect("error parsing upper left corner point");
    let lower_right = parse_complex(&args[4]).expect("error parsing lower right corner point");

    let mut pixels = vec![0; 4 * bounds.0 * bounds.1];

    // Scope of slicing up `pixels` into horizontal bands.
    {
        let bands: Vec<(usize, &mut [u8])> = pixels.chunks_mut(bounds.0 * 4).enumerate().collect();

        bands.into_par_iter().for_each(|(i, band)| {
            let top = i;
            let band_bounds = (bounds.0, 1);
            let band_upper_left = pixel_to_point(bounds, (0, top), upper_left, lower_right);
            let band_lower_right =
                pixel_to_point(bounds, (bounds.0, top + 1), upper_left, lower_right);
            render(band, band_bounds, band_upper_left, band_lower_right);
        });
    }

    write_image(&args[1], &pixels, bounds).expect("error writing PNG file");

    let (mut ctx, mut events_loop) = ContextBuilder::new("GGEZ Mandelbrot", "eisendaniel")
        .window_mode(conf::WindowMode::default().dimensions(bounds.0 as f32, bounds.1 as f32))
        .window_setup(conf::WindowSetup::default().samples(conf::NumSamples::Eight))
        .build()
        .expect("Failed to create context");

    let mut state = Mandelbrot::new(&mut ctx, args);

    match event::run(&mut ctx, &mut events_loop, &mut state) {
        Ok(_) => println!("Exited cleanly"),
        Err(e) => println!("Error: {}", e),
    }
}

struct Mandelbrot {
    win_bounds: (usize, usize),
    upper_left: Complex<f64>,
    lower_right: Complex<f64>,
    pixel_buf: Vec<u8>,
}

impl Mandelbrot {
    pub fn new(_ctx: &mut Context, args: Vec<String>) -> Mandelbrot {
        let bounds = parse_pair(&args[2], 'x').expect("error parsing image dimensions");
        Mandelbrot {
            win_bounds: bounds,
            upper_left: parse_complex(&args[3]).expect("error parsing upper left corner point"),
            lower_right: parse_complex(&args[4]).expect("error parsing lower right corner point"),
            pixel_buf: vec![0; 4 * bounds.0 * bounds.1],
        }
    }
}

impl ggez::event::EventHandler for Mandelbrot {
    fn update(&mut self, _ctx: &mut Context) -> GameResult<()> {
        {
            let bands: Vec<(usize, &mut [u8])> =
                self.pixel_buf.chunks_mut(bounds.0 * 4).enumerate().collect();

            bands.into_par_iter().for_each(|(i, band)| {
                let top = i;
                let band_bounds = (bounds.0, 1);
                let band_upper_left = pixel_to_point(bounds, (0, top), self.upper_left, self.lower_right);
                let band_lower_right =
                    pixel_to_point(bounds, (bounds.0, top + 1), self.upper_left, self.lower_right);
                render(band, band_bounds, band_upper_left, band_lower_right);
            });
        }

        Ok(())
    }
    fn draw(&mut self, ctx: &mut Context) -> GameResult<()> {
        graphics::clear(ctx, graphics::BLACK);
        let texture = graphics::Image::from_rgba8(
            ctx,
            self.win_bounds.0 as u16,
            self.win_bounds.1 as u16,
            &self.pixel_buf,
        )
        .unwrap();
        graphics::draw(ctx, &texture, graphics::DrawParam::new())?;
        graphics::present(ctx)
    }
}

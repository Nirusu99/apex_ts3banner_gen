use std::path::PathBuf;

use apex_rs::ApexClient;
use chrono::{Date, DateTime, Duration, Utc};
use config::Config;
use image::{Rgb, RgbImage};
use imageproc::drawing::draw_text_mut;
use rusttype::{Font, Scale};
const CONFIG_NAME: &'static str = "config";
const APEX_TOKEN: &'static str = "apex_token";
const FONT: &'static str = "font";
const FONT_HEIGHT: &'static str = "font_height";
const DEFAULT_FONT_HEIGHT: f32 = 14.0;
const IMAGE_NAME: &'static str = "image_name";
const DEFAULT_IMAGE_NAME: &'static str = "out.png";

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // build config from config.toml
    let config = Config::builder()
        .add_source(config::File::with_name(CONFIG_NAME))
        .build()?;

    let apex_token = config.get_string(APEX_TOKEN)?;
    let font_name = config.get_string(FONT)?;
    let image_name = config
        .get_string(IMAGE_NAME)
        .map_or(String::from(DEFAULT_IMAGE_NAME), |name| name);
    let font_height = config
        .get_float(FONT_HEIGHT)
        .map_or(DEFAULT_FONT_HEIGHT, |height| height as f32);
    let scale = Scale {
        x: font_height * 2.0,
        y: font_height,
    };
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let apex = ApexClient::new(&apex_token);
    let mut image = RgbImage::new(1000, 400);
    for x in 0..image.width() {
        for y in 0..image.height() {
            image.put_pixel(x, y, Rgb([10, 10, 10]));
        }
    }
    let maps = apex.get_map_rotations().await?;

    let current = maps.battle_royal().map_or(None, |rot| rot.current());
    let next = maps.battle_royal().map_or(None, |rot| rot.next());

    let now = Utc::now();
    let time_left = match current {
        Some(c) => {
            let end = c.end_as_date();
            end - now
        }
        None => Duration::zero(),
    };

    let mut font = root.clone();
    font.push(font_name);
    let font = match font.to_str() {
        Some(font) => font,
        None => {
            println!("Cloudn't decode {}", font.display());
            return Ok(());
        }
    };
    let font = std::fs::read(font)?;
    let font = match Font::try_from_vec(font) {
        Some(font) => font,
        None => {
            println!("Couldn't decode font");
            return Ok(());
        }
    };
    let text = format!(
        "Current Map: {}\nTime left: {} min\nNext Map: {}",
        current.map_or(String::from("unknown"), |m| m.name()),
        time_left.num_minutes(),
        next.map_or(String::from("unknown"), |m| m.name()),
    );
    let mut y = 30;
    for line in text.split('\n') {
        draw_text_mut(
            &mut image,
            Rgb([255u8, 255u8, 255u8]),
            30,
            y,
            scale,
            &font,
            &line,
        );
        y += font_height as i32;
    }
    let mut image_path = root.clone();
    image_path.push(image_name);
    image.save(image_path)?;
    Ok(())
}

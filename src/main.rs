use rayon::prelude::*;
use std::path::PathBuf;

use apex_rs::{model::Bundle, ApexClient};
use chrono::{Duration, Utc};
use config::Config;
use image::{
    imageops::{overlay, resize},
    ImageBuffer, Rgba, RgbaImage,
};
use imageproc::drawing::{draw_text_mut, text_size};
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
    let now = Utc::now();
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
    let root = std::env::current_dir()?;
    let apex = ApexClient::new(&apex_token);
    let mut image = RgbaImage::new(921, 236);
    image
        .enumerate_pixels_mut()
        .for_each(|pixel| *pixel.2 = Rgba([10, 10, 10, 255]));
    let maps = apex.get_map_rotations().await?;

    let crafter = apex.get_crafter_rotations().await?;
    let daily = convert_to_images(&convert_to_url(&crafter.daily_bundles())).await?;
    let daily_duration = crafter
        .daily_bundles()
        .first()
        .map_or(Duration::zero(), |bundle| bundle.end_as_date() - now);
    let mut weekly = convert_to_images(&convert_to_url(&crafter.weekly_bundles())).await?;
    let weekly_duration = crafter
        .weekly_bundles()
        .first()
        .map_or(Duration::zero(), |bundle| bundle.end_as_date() - now);
    let mut perma = convert_to_images(&convert_to_url_with_filter(
        &crafter.permanent_bundles(),
        |bundle: &Bundle| {
            bundle.bundle() != "ammo"
                && bundle.bundle() != "evo"
                && bundle.bundle() != "health_pickup"
                && bundle.bundle() != "shield_pickup"
        },
    ))
    .await?;
    let current = maps.battle_royal().map_or(None, |rot| rot.current());
    let next = maps.battle_royal().map_or(None, |rot| rot.next());

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
            println!("Couldn't decode {}", font.display());
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
    let lines = [
        (
            "Current Map:",
            &current.map_or(String::from("unknown"), |m| m.name()),
        ),
        (
            "Next Map:",
            &next.map_or(String::from("unknown"), |m| m.name()),
        ),
        ("Time left:", &format_duration_hhmm(time_left)),
        ("Daily Crafter:", &format_duration_hhmm(daily_duration)),
        ("Weekly Crafter:", &format_duration_ddhhmm(weekly_duration)),
    ];
    let max = lines
        .iter()
        .map(|(v, _)| v)
        // we only need the width
        .map(|s| text_size(scale, &font, s).0)
        .max()
        .unwrap_or(0);
    let text_color = Rgba([255u8, 255u8, 255u8, 255]);
    let mut y = 10;
    for (title, value) in lines {
        draw_text_mut(&mut image, text_color, 30, y, scale, &font, title);
        draw_text_mut(&mut image, text_color, 60 + max, y, scale, &font, value);
        y += font_height as i32;
    }
    y += font_height as i32;
    // insert daily images
    let mut images: Vec<_> = daily;
    images.append(&mut weekly);
    images.append(&mut perma);
    let mut x = 70;
    for i in images {
        overlay(&mut image, &i, x, y as i64);
        x += 130;
    }
    let image_path = PathBuf::from(image_name);
    image.save(image_path)?;
    Ok(())
}

fn format_duration_hhmm(duration: chrono::Duration) -> String {
    // let seconds = time_left.num_seconds() % 60;
    let minutes = (duration.num_seconds() / 60) % 60;
    let hours = duration.num_seconds() / 3600;
    format!("{:02}h {:02}m", hours, minutes)
}

fn format_duration_ddhhmm(duration: chrono::Duration) -> String {
    // let seconds = time_left.num_seconds() % 60;
    let minutes = (duration.num_seconds() / 60) % 60;
    let hours = (duration.num_seconds() / 3600) % 24;
    let days = (duration.num_seconds() / 3600) / 24;
    format!("{:02}d {:02}h {:02}m", days, hours, minutes)
}

async fn convert_to_images(
    v: &Vec<url::Url>,
) -> Result<Vec<ImageBuffer<Rgba<u8>, Vec<u8>>>, Box<dyn std::error::Error + Send + Sync>> {
    let mut cache = std::env::current_dir()?;
    cache.push("cache");
    std::fs::create_dir_all(&cache)?;
    let mut vec = Vec::new();
    for item in v {
        let file_name = item.path_segments().map(|segments| segments.last());
        let bytes = match file_name {
            Some(Some(file_name)) => {
                cache.push(file_name);
                if cache.exists() && cache.is_file() {
                    let bytes = std::fs::read(&cache)?;
                    cache.pop();
                    bytes.into()
                } else {
                    let bytes = reqwest::get(item.as_str()).await?.bytes().await?;
                    std::fs::write(&cache, &bytes)?;
                    cache.pop();
                    bytes
                }
            }
            _ => reqwest::get(item.as_str()).await?.bytes().await?,
        };
        vec.push(image::load_from_memory(&bytes)?);
    }
    let vec: Vec<ImageBuffer<Rgba<u8>, Vec<u8>>> = vec
        .par_iter()
        .map(|i| resize(i, 100, 100, image::imageops::FilterType::Lanczos3))
        .collect();
    Ok(vec)
}

fn convert_to_url(v: &Vec<&Bundle>) -> Vec<url::Url> {
    v.iter()
        .flat_map(|bundle| bundle.items())
        .map(|item| item.item_type().asset_as_url())
        .filter(|item| item.is_ok())
        .map(|item| item.unwrap())
        .collect()
}
fn convert_to_url_with_filter<F>(v: &Vec<&Bundle>, f: F) -> Vec<url::Url>
where
    F: Fn(&Bundle) -> bool,
{
    v.iter()
        .filter(|item| f(item))
        .flat_map(|bundle| bundle.items())
        .map(|item| item.item_type().asset_as_url())
        .filter(|item| item.is_ok())
        .map(|item| item.unwrap())
        .collect()
}

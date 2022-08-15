use rayon::prelude::*;
use std::path::PathBuf;

use apex_rs::ApexClient;
use chrono::{Duration, Utc};
use config::Config;
use image::{
    imageops::{overlay, resize},
    ImageBuffer, Rgba, RgbaImage,
};
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
    let mut image = RgbaImage::new(1000, 630);
    for x in 0..image.width() {
        for y in 0..image.height() {
            image.put_pixel(x, y, Rgba([10, 10, 10, 255]));
        }
    }
    let maps = apex.get_map_rotations().await?;

    let now = Utc::now();
    let crafter = apex.get_crafter_rotations().await?;
    let dailies: Vec<_> = crafter
        .daily_bundles()
        .iter()
        .flat_map(|bundle| bundle.items())
        .map(|item| item.item_type().asset())
        .collect();
    let dailies = convert_to_images(&dailies).await?;
    let daily_duration = crafter
        .daily_bundles()
        .first()
        .map_or(Duration::zero(), |bundle| bundle.end_as_date() - now);
    let weekly: Vec<_> = crafter
        .weekly_bundles()
        .iter()
        .flat_map(|bundle| bundle.items())
        .map(|item| item.item_type().asset())
        .collect();
    let weekly = convert_to_images(&weekly).await?;
    let weekly_duration = crafter
        .weekly_bundles()
        .first()
        .map_or(Duration::zero(), |bundle| bundle.end_as_date() - now);
    let perma: Vec<_> = crafter
        .permanent_bundles()
        .iter()
        .filter(|bundle| {
            bundle.bundle() != "ammo"
                && bundle.bundle() != "evo"
                && bundle.bundle() != "health_pickup"
                && bundle.bundle() != "shield_pickup"
        })
        .flat_map(|bundle| bundle.items())
        .map(|item| item.item_type().asset())
        .collect();
    let perma = convert_to_images(&perma).await?;
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
    let text = format!(
        "Current Map: {}\nTime left:   {}\nNext Map:    {}",
        current.map_or(String::from("unknown"), |m| m.name()),
        format_duration_hhmm(time_left),
        next.map_or(String::from("unknown"), |m| m.name()),
    );
    let mut y = 30;
    for line in text.split('\n') {
        draw_text_mut(
            &mut image,
            Rgba([255u8, 255u8, 255u8, 255]),
            30,
            y,
            scale,
            &font,
            &line,
        );
        y += font_height as i32;
    }
    y += font_height as i32;
    draw_text_mut(
        &mut image,
        Rgba([255u8, 255u8, 255u8, 255]),
        30,
        y,
        scale,
        &font,
        &format!("Daily Crafter ({}):", format_duration_hhmm(daily_duration)),
    );
    y += font_height as i32;
    // insert daily images
    let mut x = 30;
    for i in dailies {
        overlay(&mut image, &i, x, y as i64);
        x += 130;
    }
    y += font_height as i32 + 100;
    draw_text_mut(
        &mut image,
        Rgba([255u8, 255u8, 255u8, 255]),
        30,
        y,
        scale,
        &font,
        &format!(
            "Weekly Crafter ({}):",
            format_duration_ddhhmm(weekly_duration)
        ),
    );
    y += font_height as i32;
    // insert weekly images
    let mut x = 30;
    for i in weekly {
        overlay(&mut image, &i, x, y as i64);
        x += 130;
    }
    y += font_height as i32 + 100;
    draw_text_mut(
        &mut image,
        Rgba([255u8, 255u8, 255u8, 255]),
        30,
        y,
        scale,
        &font,
        "Permanent Crafter:",
    );
    y += font_height as i32;
    // insert perma images
    let mut x = 30;
    for i in perma {
        overlay(&mut image, &i, x, y as i64);
        x += 130;
    }
    let mut image_path = root.clone();
    image_path.push(image_name);
    image.save(image_path)?;
    Ok(())
}

fn format_duration_hhmm(duration: chrono::Duration) -> String {
    // let seconds = time_left.num_seconds() % 60;
    let minutes = (duration.num_seconds() / 60) % 60;
    let hours = duration.num_seconds() / 3600;
    format!("{:02}:{:02}", hours, minutes)
}

fn format_duration_ddhhmm(duration: chrono::Duration) -> String {
    // let seconds = time_left.num_seconds() % 60;
    let minutes = (duration.num_seconds() / 60) % 60;
    let hours = (duration.num_seconds() / 3600) % 24;
    let days = (duration.num_seconds() / 3600) / 24;
    format!("{:02}:{:02}:{:02}", days, hours, minutes)
}

async fn convert_to_images(
    v: &Vec<&str>,
) -> Result<Vec<ImageBuffer<Rgba<u8>, Vec<u8>>>, Box<dyn std::error::Error + Send + Sync>> {
    let mut vec = Vec::new();
    for item in v {
        let bytes = reqwest::get(*item).await?.bytes().await?;
        vec.push(image::load_from_memory(&bytes)?);
    }
    let vec: Vec<ImageBuffer<Rgba<u8>, Vec<u8>>> = vec
        .par_iter()
        .map(|i| resize(i, 100, 100, image::imageops::FilterType::Nearest))
        .collect();
    Ok(vec)
}

use anyhow::{anyhow, Result};
use serde::Serialize;

#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
pub struct RgbColor {
    pub r: f32,
    pub g: f32,
    pub b: f32,
}

impl RgbColor {
    pub fn parse(raw: &str) -> Result<Self> {
        let raw = raw.trim();
        if raw.is_empty() {
            return Err(anyhow!("empty color value"));
        }
        if let Some(hex) = raw.strip_prefix('#') {
            return parse_hex(hex);
        }
        match raw.to_ascii_lowercase().as_str() {
            "black" => Ok(Self::new_u8(0, 0, 0)),
            "white" => Ok(Self::new_u8(255, 255, 255)),
            "red" => Ok(Self::new_u8(255, 0, 0)),
            "blue" => Ok(Self::new_u8(0, 0, 255)),
            "green" => Ok(Self::new_u8(0, 128, 0)),
            "gray" | "grey" => Ok(Self::new_u8(128, 128, 128)),
            "darkgray" | "darkgrey" => Ok(Self::new_u8(64, 64, 64)),
            "lightgray" | "lightgrey" => Ok(Self::new_u8(200, 200, 200)),
            other => Err(anyhow!("unsupported color '{}'", other)),
        }
    }

    pub fn new_u8(r: u8, g: u8, b: u8) -> Self {
        Self {
            r: r as f32 / 255.0,
            g: g as f32 / 255.0,
            b: b as f32 / 255.0,
        }
    }
}

fn parse_hex(hex: &str) -> Result<RgbColor> {
    let chars: Vec<char> = hex.chars().collect();
    match chars.len() {
        3 => {
            let r = parse_hex_component(chars[0])?;
            let g = parse_hex_component(chars[1])?;
            let b = parse_hex_component(chars[2])?;
            Ok(RgbColor::new_u8(r, g, b))
        }
        6 => {
            let r = u8::from_str_radix(&hex[0..2], 16)?;
            let g = u8::from_str_radix(&hex[2..4], 16)?;
            let b = u8::from_str_radix(&hex[4..6], 16)?;
            Ok(RgbColor::new_u8(r, g, b))
        }
        _ => Err(anyhow!("invalid hex color '#{}'", hex)),
    }
}

fn parse_hex_component(ch: char) -> Result<u8> {
    let s = format!("{ch}{ch}");
    Ok(u8::from_str_radix(&s, 16)?)
}

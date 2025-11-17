#![allow(dead_code)]

use crate::color::RgbColor;
use anyhow::{Context, Result, anyhow};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::str::FromStr;

#[derive(Debug, Clone)]
pub struct RawConfig {
    source: PathBuf,
    data: BTreeMap<String, String>,
}

impl RawConfig {
    pub fn load(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        let content =
            fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
        let mut data = BTreeMap::new();
        for raw_line in content.lines() {
            if let Some((k, v)) = parse_line(raw_line) {
                data.insert(k, v);
            }
        }
        Ok(Self {
            source: path.to_path_buf(),
            data,
        })
    }

    pub fn get(&self, key: &str) -> Option<&str> {
        self.data.get(key).map(|s| s.as_str())
    }

    pub fn require(&self, key: &str) -> Result<&str> {
        self.get(key)
            .ok_or_else(|| anyhow!("missing key '{}' in {}", key, self.source.display()))
    }

    pub fn parse_value<T>(&self, key: &str) -> Result<T>
    where
        T: FromStr,
        <T as FromStr>::Err: std::fmt::Display,
    {
        let raw = self.require(key)?;
        raw.parse::<T>()
            .map_err(|err| anyhow!("{}: parse {} -> {}", self.source.display(), key, err))
    }
}

fn parse_line(raw: &str) -> Option<(String, String)> {
    let trimmed = raw.trim();
    if trimmed.is_empty() || trimmed.starts_with('#') {
        return None;
    }

    let without_inline = if trimmed.contains("=#") {
        trimmed
    } else {
        trimmed.split('#').next().unwrap_or("").trim()
    };

    if without_inline.is_empty() {
        return None;
    }

    let collapsed: String = without_inline
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect();
    let mut parts = collapsed.splitn(2, '=');
    let key = parts.next()?.to_string();
    let value = parts
        .next()
        .map(|v| v.to_string())
        .unwrap_or_else(String::new);
    if key.is_empty() {
        None
    } else {
        Some((key, value))
    }
}

#[derive(Debug, Clone)]
pub struct FontSlot {
    pub id: usize,
    pub name: String,
    pub rotate_deg: f32,
    pub text_size: f32,
    pub comment_size: f32,
}

#[derive(Debug, Clone)]
pub struct FontMapping {
    pub slots: Vec<Option<FontSlot>>,
    pub text_stack: Vec<usize>,
    pub comment_stack: Vec<usize>,
}

#[derive(Debug, Clone)]
pub struct CoverConfig {
    pub title_font_size: f32,
    pub title_y: f32,
    pub author_font_size: f32,
    pub author_y: f32,
    pub color: RgbColor,
}

#[derive(Debug, Clone)]
pub struct TitleConfig {
    pub center: bool,
    pub postfix: Option<String>,
    pub directory: bool,
    pub font_size: f32,
    pub color: RgbColor,
    pub y: f32,
    pub y_dis: f32,
}

#[derive(Debug, Clone)]
pub struct PagerConfig {
    pub font_size: f32,
    pub color: RgbColor,
    pub y: f32,
}

#[derive(Debug, Clone)]
pub struct ReplacementRules {
    pub comma_pairs: Vec<(char, String)>,
    pub number_pairs: Vec<(char, String)>,
    pub delete_tokens: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct TextModes {
    pub remove_punctuations: bool,
    pub remove_tokens: Vec<String>,
    pub only_period: bool,
    pub only_period_tokens: Vec<String>,
    pub only_period_color: Option<RgbColor>,
}

#[derive(Debug, Clone)]
pub struct MarkAdjust {
    pub chars: Vec<char>,
    pub scale: f32,
    pub offset_x: f32,
    pub offset_y: f32,
}

impl MarkAdjust {
    pub fn empty() -> Self {
        Self {
            chars: Vec::new(),
            scale: 1.0,
            offset_x: 0.0,
            offset_y: 0.0,
        }
    }
}

#[derive(Debug, Clone)]
pub struct PunctuationConfig {
    pub text_nop: MarkAdjust,
    pub text_rotate: MarkAdjust,
    pub comment_nop: MarkAdjust,
    pub comment_rotate: MarkAdjust,
    pub comment_strip_chars: Vec<char>,
}

#[derive(Debug, Clone)]
pub struct BookLineConfig {
    pub width: f32,
    pub color: RgbColor,
}

#[derive(Debug, Clone)]
pub struct BookConfig {
    pub title: String,
    pub author: String,
    pub canvas_id: String,
    pub row_num: usize,
    pub row_delta_y: f32,
    pub multirows_horizontal_layout: usize,
    pub fonts: FontMapping,
    pub try_st: bool,
    pub text_font_color: RgbColor,
    pub comment_font_color: RgbColor,
    pub cover: CoverConfig,
    pub title_style: TitleConfig,
    pub pager_style: PagerConfig,
    pub replacements: ReplacementRules,
    pub text_modes: TextModes,
    pub punctuation: PunctuationConfig,
    pub bookline: Option<BookLineConfig>,
    pub book_line_flag: bool,
}

impl BookConfig {
    pub fn load(path: impl AsRef<Path>) -> Result<Self> {
        let raw = RawConfig::load(path)?;
        let title = raw.require("title")?.to_string();
        let author = raw.require("author")?.to_string();
        let canvas_id = raw.require("canvas_id")?.to_string();
        let row_num = raw.parse_value::<usize>("row_num")?;
        let row_delta_y = raw
            .get("row_delta_y")
            .unwrap_or("0")
            .parse::<f32>()
            .unwrap_or(0.0);
        let multirows_horizontal_layout = raw
            .get("multirows_horizontal_layout")
            .and_then(|s| s.parse::<usize>().ok())
            .unwrap_or(1);

        let fonts = parse_font_mapping(&raw)?;
        if fonts.text_stack.is_empty() {
            return Err(anyhow!("text_fonts_array is empty"));
        }
        if fonts.comment_stack.is_empty() {
            return Err(anyhow!("comment_fonts_array is empty"));
        }

        let try_st = parse_bool(raw.get("try_st"));
        let text_font_color = parse_color(raw.get("text_font_color"), RgbColor::new_u8(0, 0, 0))?;
        let comment_font_color =
            parse_color(raw.get("comment_font_color"), RgbColor::new_u8(0, 0, 0))?;

        let cover = CoverConfig {
            title_font_size: parse_f32(raw.get("cover_title_font_size"), 120.0)?,
            title_y: parse_f32(raw.get("cover_title_y"), 200.0)?,
            author_font_size: parse_f32(raw.get("cover_author_font_size"), 60.0)?,
            author_y: parse_f32(raw.get("cover_author_y"), 600.0)?,
            color: parse_color(raw.get("cover_font_color"), RgbColor::new_u8(0, 0, 0))?,
        };

        let title_style = TitleConfig {
            center: parse_bool(raw.get("if_tpcenter")),
            postfix: parse_optional_string(raw.get("title_postfix")),
            directory: parse_bool(raw.get("title_directory")),
            font_size: parse_f32(raw.get("title_font_size"), 80.0)?,
            color: parse_color(raw.get("title_font_color"), RgbColor::new_u8(0, 0, 0))?,
            y: parse_f32(raw.get("title_y"), 1200.0)?,
            y_dis: parse_f32(raw.get("title_ydis"), 1.2)?,
        };

        let pager_style = PagerConfig {
            font_size: parse_f32(raw.get("pager_font_size"), 35.0)?,
            color: parse_color(raw.get("pager_font_color"), RgbColor::new_u8(0, 0, 0))?,
            y: parse_f32(raw.get("pager_y"), 500.0)?,
        };

        let replacements = ReplacementRules {
            comma_pairs: parse_replace_pairs(raw.get("exp_replace_comma")),
            number_pairs: parse_replace_pairs(raw.get("exp_replace_number")),
            delete_tokens: parse_token_list(raw.get("exp_delete_comma")),
        };

        let text_modes = TextModes {
            remove_punctuations: parse_bool(raw.get("if_nocomma")),
            remove_tokens: parse_token_list(raw.get("exp_nocomma")),
            only_period: parse_bool(raw.get("if_onlyperiod")),
            only_period_tokens: parse_token_list(raw.get("exp_onlyperiod")),
            only_period_color: raw
                .get("onlyperiod_color")
                .filter(|s| !s.is_empty())
                .map(|s| RgbColor::parse(s))
                .transpose()?,
        };

        let punctuation = PunctuationConfig {
            text_nop: parse_mark_adjust(
                raw.get("text_comma_nop"),
                raw.get("text_comma_nop_size"),
                raw.get("text_comma_nop_x"),
                raw.get("text_comma_nop_y"),
            )?,
            text_rotate: parse_mark_adjust(
                raw.get("text_comma_90"),
                raw.get("text_comma_90_size"),
                raw.get("text_comma_90_x"),
                raw.get("text_comma_90_y"),
            )?,
            comment_nop: parse_mark_adjust(
                raw.get("comment_comma_nop"),
                raw.get("comment_comma_nop_size"),
                raw.get("comment_comma_nop_x"),
                raw.get("comment_comma_nop_y"),
            )?,
            comment_rotate: parse_mark_adjust(
                raw.get("comment_comma_90"),
                raw.get("comment_comma_90_size"),
                raw.get("comment_comma_90_x"),
                raw.get("comment_comma_90_y"),
            )?,
            comment_strip_chars: parse_char_list_from_pipe(raw.get("comment_comma_nop")),
        };

        let book_line_flag = parse_bool(raw.get("if_book_vline"));
        let bookline = if book_line_flag {
            Some(BookLineConfig {
                width: parse_f32(raw.get("book_line_width"), 1.0)?,
                color: parse_color(raw.get("book_line_color"), RgbColor::new_u8(0, 0, 0))?,
            })
        } else {
            None
        };

        Ok(Self {
            title,
            author,
            canvas_id,
            row_num,
            row_delta_y,
            multirows_horizontal_layout,
            fonts,
            try_st,
            text_font_color,
            comment_font_color,
            cover,
            title_style,
            pager_style,
            replacements,
            text_modes,
            punctuation,
            bookline,
            book_line_flag,
        })
    }

    pub fn validate(&self) -> Result<()> {
        if self.row_num == 0 {
            return Err(anyhow!("row_num must be > 0"));
        }
        if self.fonts.text_stack.is_empty() {
            return Err(anyhow!("text_fonts_array must not be empty"));
        }
        if self.cover.title_font_size <= 0.0 || self.cover.author_font_size <= 0.0 {
            return Err(anyhow!("cover font sizes must be > 0"));
        }
        if self.title_style.font_size <= 0.0 {
            return Err(anyhow!("title font size must be > 0"));
        }
        if self.pager_style.font_size <= 0.0 {
            return Err(anyhow!("pager font size must be > 0"));
        }
        Ok(())
    }
}

fn parse_font_mapping(raw: &RawConfig) -> Result<FontMapping> {
    let mut slots = Vec::new();
    for idx in 1..=5 {
        let key = format!("font{idx}");
        let name = raw.get(&key).unwrap_or("").to_string();
        if name.is_empty() {
            slots.push(None);
            continue;
        }
        let rotate = parse_f32(raw.get(&format!("font{idx}_rotate")), 0.0)?;
        let text_size = parse_f32(raw.get(&format!("text_font{idx}_size")), 60.0)?;
        let comment_size = parse_f32(raw.get(&format!("comment_font{idx}_size")), 30.0)?;
        slots.push(Some(FontSlot {
            id: idx,
            name,
            rotate_deg: rotate,
            text_size,
            comment_size,
        }));
    }

    let parse_stack = |raw_value: Option<&str>| -> Vec<usize> {
        raw_value
            .unwrap_or("")
            .chars()
            .filter_map(|c| c.to_digit(10).map(|d| d as usize))
            .filter(|&idx| idx >= 1 && idx <= 5)
            .collect()
    };

    let text_stack = parse_stack(raw.get("text_fonts_array"));
    let comment_stack = parse_stack(raw.get("comment_fonts_array"));

    Ok(FontMapping {
        slots,
        text_stack,
        comment_stack,
    })
}

fn parse_color(value: Option<&str>, default: RgbColor) -> Result<RgbColor> {
    match value {
        Some(v) if !v.is_empty() => RgbColor::parse(v),
        _ => Ok(default),
    }
}

fn parse_bool(value: Option<&str>) -> bool {
    matches!(value, Some(v) if v.trim() == "1")
}

fn parse_f32(value: Option<&str>, default: f32) -> Result<f32> {
    match value {
        Some(v) if !v.is_empty() => Ok(v.parse::<f32>().unwrap_or(default)),
        _ => Ok(default),
    }
}

fn parse_optional_string(value: Option<&str>) -> Option<String> {
    value
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
}

fn parse_replace_pairs(value: Option<&str>) -> Vec<(char, String)> {
    value
        .unwrap_or("")
        .split('|')
        .filter_map(|pair| {
            if pair.is_empty() {
                None
            } else {
                let mut chars = pair.chars();
                let key = chars.next()?;
                let val: String = chars.collect();
                if val.is_empty() {
                    None
                } else {
                    Some((key, val))
                }
            }
        })
        .collect()
}

fn parse_token_list(value: Option<&str>) -> Vec<String> {
    value
        .unwrap_or("")
        .split('|')
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .collect()
}

fn parse_char_list_from_pipe(value: Option<&str>) -> Vec<char> {
    value
        .unwrap_or("")
        .split('|')
        .filter_map(|token| token.chars().next())
        .collect()
}

fn parse_mark_adjust(
    chars_raw: Option<&str>,
    scale_raw: Option<&str>,
    ox_raw: Option<&str>,
    oy_raw: Option<&str>,
) -> Result<MarkAdjust> {
    let chars = if let Some(raw) = chars_raw {
        if raw.contains('|') {
            parse_char_list_from_pipe(chars_raw)
        } else {
            raw.chars().collect()
        }
    } else {
        Vec::new()
    };
    Ok(MarkAdjust {
        chars,
        scale: parse_f32(scale_raw, 1.0)?,
        offset_x: parse_f32(ox_raw, 0.0)?,
        offset_y: parse_f32(oy_raw, 0.0)?,
    })
}

#[derive(Debug, Clone)]
pub struct CanvasConfig {
    pub canvas_width: f32,
    pub canvas_height: f32,
    pub margins_top: f32,
    pub margins_bottom: f32,
    pub margins_left: f32,
    pub margins_right: f32,
    pub leaf_col: usize,
    pub leaf_center_width: f32,
    pub logo_text: Option<String>,
    pub multirows_enabled: bool,
    pub multirows_count: usize,
}

impl CanvasConfig {
    pub fn load(path: impl AsRef<Path>) -> Result<Self> {
        let raw = RawConfig::load(path)?;
        let getf = |key: &str| -> Result<f32> { raw.parse_value::<f32>(key) };
        Ok(Self {
            canvas_width: getf("canvas_width")?,
            canvas_height: getf("canvas_height")?,
            margins_top: getf("margins_top")?,
            margins_bottom: getf("margins_bottom")?,
            margins_left: getf("margins_left")?,
            margins_right: getf("margins_right")?,
            leaf_col: raw.parse_value::<usize>("leaf_col")?,
            leaf_center_width: getf("leaf_center_width")?,
            logo_text: parse_optional_string(raw.get("logo_text")),
            multirows_enabled: parse_bool(raw.get("if_multirows")),
            multirows_count: raw
                .get("multirows_num")
                .and_then(|s| s.parse::<usize>().ok())
                .unwrap_or(1),
        })
    }

    pub fn validate(&self) -> Result<()> {
        if self.canvas_width <= 0.0 || self.canvas_height <= 0.0 {
            return Err(anyhow!("canvas_width/height must be > 0"));
        }
        if self.leaf_col == 0 {
            return Err(anyhow!("leaf_col must be > 0"));
        }
        if self.margins_left + self.margins_right >= self.canvas_width {
            return Err(anyhow!(
                "left+right margins ({}) exceed canvas width ({})",
                self.margins_left + self.margins_right,
                self.canvas_width
            ));
        }
        if self.margins_top + self.margins_bottom >= self.canvas_height {
            return Err(anyhow!(
                "top+bottom margins ({}) exceed canvas height ({})",
                self.margins_top + self.margins_bottom,
                self.canvas_height
            ));
        }
        Ok(())
    }
}

#![allow(dead_code)]

use crate::config::{BookConfig, FontSlot};
use anyhow::{Context, Result, anyhow};
use fontdue::Font;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct LoadedFont {
    pub slot: FontSlot,
    pub data: Vec<u8>,
    pub font: Font,
    pub path: PathBuf,
}

#[derive(Debug)]
pub struct FontManager {
    pub slots: Vec<Option<LoadedFont>>,
    pub text_stack: Vec<usize>,
    pub comment_stack: Vec<usize>,
}

impl FontManager {
    pub fn new(book: &BookConfig, fonts_root: &Path) -> Result<Self> {
        let mut slots = Vec::with_capacity(book.fonts.slots.len());
        for slot in &book.fonts.slots {
            if let Some(slot_info) = slot {
                let path = fonts_root.join(&slot_info.name);
                let data =
                    fs::read(&path).with_context(|| format!("loading font {}", path.display()))?;
                let font = Font::from_bytes(data.clone(), fontdue::FontSettings::default())
                    .map_err(|err| anyhow!("{}: {}", slot_info.name, err))?;
                slots.push(Some(LoadedFont {
                    slot: slot_info.clone(),
                    data,
                    font,
                    path,
                }));
            } else {
                slots.push(None);
            }
        }
        Ok(Self {
            slots,
            text_stack: book.fonts.text_stack.clone(),
            comment_stack: book.fonts.comment_stack.clone(),
        })
    }

    pub fn font(&self, idx: usize) -> Option<&LoadedFont> {
        if idx == 0 {
            return None;
        }
        self.slots.get(idx - 1)?.as_ref()
    }

    pub fn has_glyph(&self, font_idx: usize, ch: char) -> bool {
        self.font(font_idx)
            .map(|lf| lf.font.lookup_glyph_index(ch) != 0)
            .unwrap_or(false)
    }

    pub fn pick_font(&self, ch: char, stack: &[usize]) -> Option<FontPick<'_>> {
        for &idx in stack {
            if self.has_glyph(idx, ch) {
                if let Some(font) = self.font(idx) {
                    return Some(FontPick {
                        font,
                        slot_index: idx,
                    });
                }
            }
        }
        None
    }
}

pub struct FontPick<'a> {
    pub font: &'a LoadedFont,
    pub slot_index: usize,
}

use crate::color::RgbColor;
use anyhow::{Result, anyhow};
use serde::Serialize;
use std::collections::HashSet;
use std::fs::File;
use std::io::BufWriter;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize)]
pub enum CoverPlan {
    Image,
    Generated,
}

#[derive(Debug, Clone, Serialize)]
pub struct GlyphSpec {
    pub ch: char,
    pub font_idx: usize,
    pub font_size: f32,
    pub x: f32,
    pub y: f32,
    pub rotate_deg: f32,
    pub color: RgbColor,
}

#[derive(Debug, Clone, Serialize)]
pub struct LineSpec {
    pub x1: f32,
    pub y1: f32,
    pub x2: f32,
    pub y2: f32,
    pub width: f32,
    pub color: RgbColor,
    pub wavy: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct PagePlan {
    pub number: usize,
    pub title: String,
    pub glyphs: Vec<GlyphSpec>,
    pub lines: Vec<LineSpec>,
}

#[derive(Debug, Clone, Serialize)]
pub struct OutlineEntry {
    pub title: String,
    pub page_number: usize,
}

#[derive(Debug, Serialize)]
pub struct DocumentPlan {
    pub cover: CoverPlan,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cover_path: Option<PathBuf>,
    pub pages: Vec<PagePlan>,
    pub outlines: Vec<OutlineEntry>,
}

#[derive(Clone)]
pub struct TypesetOptions {
    pub from: usize,
    pub to: usize,
    pub test_pages: Option<usize>,
    pub verbose: bool,
    pub cover_image: Option<PathBuf>,
}

impl DocumentPlan {
    pub fn write_debug_json<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        let file = File::create(path)?;
        let writer = BufWriter::new(file);
        serde_json::to_writer_pretty(writer, self)?;
        Ok(())
    }

    pub fn validate(&self) -> Result<()> {
        if matches!(self.cover, CoverPlan::Image) && self.cover_path.is_none() {
            return Err(anyhow!(
                "cover plan requests an image but no cover_path was recorded"
            ));
        }
        let mut last_page = 0usize;
        let mut seen_pages = HashSet::new();
        for page in &self.pages {
            if page.number == 0 {
                return Err(anyhow!("page number cannot be 0"));
            }
            if page.number <= last_page {
                return Err(anyhow!(
                    "page numbers must be strictly increasing ({} -> {})",
                    last_page,
                    page.number
                ));
            }
            last_page = page.number;
            seen_pages.insert(page.number);
        }
        for outline in &self.outlines {
            if !seen_pages.contains(&outline.page_number) {
                return Err(anyhow!(
                    "outline '{}' references missing page {}",
                    outline.title,
                    outline.page_number
                ));
            }
        }
        Ok(())
    }
}

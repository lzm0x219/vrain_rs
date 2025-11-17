use crate::color::RgbColor;
use crate::config::{BookConfig, CanvasConfig};
use crate::fonts::FontManager;
use crate::numerals::NumeralMap;
use crate::plan::{CoverPlan, DocumentPlan, GlyphSpec, LineSpec, PagePlan};
use anyhow::{Result, anyhow, Context};
use image::DynamicImage;
use printpdf::{
    Color, FontId, Line, LinePoint, Mm, Op, ParsedFont, PdfDocument, PdfPage, PdfSaveOptions,
    Point, Pt, Px, RawImage, RawImageData, RawImageFormat, Rgb, TextItem, TextMatrix, XObjectId,
    XObjectTransform,
};
use std::collections::HashMap;
use std::fs::File;
use std::io::BufWriter;
use std::path::{Path, PathBuf};

const PX_TO_MM: f32 = 25.4f32 / 72.0f32;
const IMAGE_DPI: f32 = 72.0f32;

pub struct RenderContext<'a> {
    pub book: &'a BookConfig,
    pub canvas: &'a CanvasConfig,
    pub fonts: &'a FontManager,
    pub numerals: &'a NumeralMap,
    pub background: Option<DynamicImage>,
    pub cover_image: Option<DynamicImage>,
}

pub fn render_document(plan: &DocumentPlan, ctx: &RenderContext, output_path: &Path) -> Result<()> {
    let width_mm = Mm(ctx.canvas.canvas_width * PX_TO_MM);
    let height_mm = Mm(ctx.canvas.canvas_height * PX_TO_MM);

    let mut doc = PdfDocument::new("vRain");
    let font_ids = prepare_font_ids(&mut doc, ctx.fonts)?;
    let outline_map = build_outline_map(plan, ctx);
    let stamps = load_stamps(ctx, output_path)?;

    let background_image_id = if let Some(image) = ctx.background.as_ref() {
        Some(register_image(&mut doc, image))
    } else {
        None
    }
    .transpose()?;

    let cover_image_id = if matches!(plan.cover, CoverPlan::Image) {
        ctx.cover_image
            .as_ref()
            .map(|image| register_image(&mut doc, image))
            .transpose()?
    } else {
        None
    };

    let mut pages = Vec::with_capacity(plan.pages.len() + 1);
    let cover_ops = build_cover_ops(
        plan,
        ctx,
        &font_ids,
        background_image_id.as_ref(),
        cover_image_id.as_ref(),
    )?;
    pages.push(PdfPage::new(width_mm, height_mm, cover_ops));

    for page in &plan.pages {
        let ops = build_page_ops(
            page,
            ctx,
            &font_ids,
            background_image_id.as_ref(),
            stamps.get(&page.number),
            &mut doc,
        )?;
        pages.push(PdfPage::new(width_mm, height_mm, ops));
        if let Some(outlines) = outline_map.as_ref().and_then(|map| map.get(&page.number)) {
            let pdf_page_number = pages.len();
            for title in outlines {
                doc.add_bookmark(title, pdf_page_number);
            }
        }
    }

    doc.with_pages(pages);
    let file = File::create(output_path)?;
    let mut writer = BufWriter::new(file);
    let mut warnings = Vec::new();
    doc.save_writer(&mut writer, &PdfSaveOptions::default(), &mut warnings);
    Ok(())
}

fn prepare_font_ids(doc: &mut PdfDocument, fonts: &FontManager) -> Result<Vec<Option<FontId>>> {
    let mut font_ids = Vec::with_capacity(fonts.slots.len());
    for slot in &fonts.slots {
        if let Some(font) = slot {
            let mut warnings = Vec::new();
            let parsed = ParsedFont::from_bytes(&font.data, 0, &mut warnings)
                .ok_or_else(|| anyhow!("failed to load font '{}'", font.slot.name))?;
            let font_id = doc.add_font(&parsed);
            font_ids.push(Some(font_id));
        } else {
            font_ids.push(None);
        }
    }
    Ok(font_ids)
}

fn build_cover_ops(
    plan: &DocumentPlan,
    ctx: &RenderContext,
    font_ids: &[Option<FontId>],
    background: Option<&XObjectId>,
    cover_image: Option<&XObjectId>,
) -> Result<Vec<Op>> {
    let mut ops = Vec::new();
    match (&plan.cover, cover_image) {
        (CoverPlan::Image, Some(image_id)) => {
            push_full_page_image(&mut ops, image_id);
        }
        (CoverPlan::Image, None) => {
            if let Some(path) = &plan.cover_path {
                eprintln!(
                    "Cover image requested ({}) but no image loaded; fallback to generated cover",
                    path.display()
                );
            } else {
                eprintln!("Cover image requested but no image loaded; fallback to generated cover");
            }
            add_background_ops(&mut ops, background);
            draw_simple_cover(ctx, &mut ops, font_ids)?;
        }
        _ => {
            add_background_ops(&mut ops, background);
            draw_simple_cover(ctx, &mut ops, font_ids)?;
        }
    }
    Ok(ops)
}

fn build_page_ops(
    page: &PagePlan,
    ctx: &RenderContext,
    font_ids: &[Option<FontId>],
    background: Option<&XObjectId>,
    stamps: Option<&Vec<StampSpec>>,
    doc: &mut PdfDocument,
) -> Result<Vec<Op>> {
    let mut ops = Vec::new();
    add_background_ops(&mut ops, background);
    if let Some(stamps) = stamps {
        for stamp in stamps {
            add_stamp(&mut ops, stamp, ctx, doc)?;
        }
    }
    draw_page_title(ctx, &mut ops, font_ids, &page.title);
    draw_page_number(ctx, &mut ops, font_ids, page.number);
    for line in &page.lines {
        draw_line(&mut ops, line);
    }
    for glyph in &page.glyphs {
        draw_glyph(&mut ops, font_ids, glyph)?;
    }
    Ok(ops)
}

fn add_background_ops(ops: &mut Vec<Op>, image_id: Option<&XObjectId>) {
    if let Some(id) = image_id {
        push_full_page_image(ops, id);
    }
}

fn push_full_page_image(ops: &mut Vec<Op>, image_id: &XObjectId) {
    ops.push(Op::UseXobject {
        id: image_id.clone(),
        transform: XObjectTransform {
            translate_x: Some(Mm(0.0).into()),
            translate_y: Some(Mm(0.0).into()),
            dpi: Some(IMAGE_DPI),
            ..Default::default()
        },
    });
}

fn draw_simple_cover(
    ctx: &RenderContext,
    ops: &mut Vec<Op>,
    font_ids: &[Option<FontId>],
) -> Result<()> {
    if let Some(font_idx) = ctx.fonts.text_stack.first().copied() {
        if let Some(font_id) = font_id(font_ids, font_idx) {
            for (idx, ch) in ctx.book.title.chars().enumerate() {
                let x = ctx.book.cover.title_font_size;
                let y = ctx.canvas.canvas_height
                    - ctx.book.cover.title_y
                    - idx as f32 * ctx.book.cover.title_font_size * 1.2;
                push_text_ops(
                    ops,
                    &font_id,
                    &ctx.book.cover.color,
                    ctx.book.cover.title_font_size,
                    x,
                    y,
                    0.0,
                    &ch.to_string(),
                );
            }
            for (idx, ch) in ctx.book.author.chars().enumerate() {
                let x = ctx.book.cover.author_font_size / 2.0;
                let y = ctx.canvas.canvas_height
                    - ctx.book.cover.author_y
                    - idx as f32 * ctx.book.cover.author_font_size * 1.2;
                push_text_ops(
                    ops,
                    &font_id,
                    &ctx.book.cover.color,
                    ctx.book.cover.author_font_size,
                    x,
                    y,
                    0.0,
                    &ch.to_string(),
                );
            }
        }
    }
    Ok(())
}

fn draw_page_title(
    ctx: &RenderContext,
    ops: &mut Vec<Op>,
    font_ids: &[Option<FontId>],
    title: &str,
) {
    if let Some(font_idx) = ctx.fonts.text_stack.first().copied() {
        if let Some(font_id) = font_id(font_ids, font_idx) {
            for (idx, ch) in title.chars().enumerate() {
                let x = if ctx.book.title_style.center {
                    ctx.canvas.canvas_width / 2.0 - ctx.book.title_style.font_size / 2.0
                } else {
                    0.0
                };
                let y = ctx.book.title_style.y
                    - ctx.book.title_style.font_size * idx as f32 * ctx.book.title_style.y_dis;
                push_text_ops(
                    ops,
                    &font_id,
                    &ctx.book.title_style.color,
                    ctx.book.title_style.font_size,
                    x,
                    y,
                    0.0,
                    &ch.to_string(),
                );
            }
        }
    }
}

fn draw_page_number(
    ctx: &RenderContext,
    ops: &mut Vec<Op>,
    font_ids: &[Option<FontId>],
    number: usize,
) {
    if let Some(font_idx) = ctx.fonts.text_stack.first().copied() {
        if let Some(font_id) = font_id(font_ids, font_idx) {
            let text = ctx.numerals.render(number);
            for (idx, ch) in text.chars().enumerate() {
                let x = ctx.canvas.canvas_width / 2.0 - ctx.book.pager_style.font_size / 2.0;
                let y = ctx.book.pager_style.y
                    - ctx.book.pager_style.font_size * idx as f32 * ctx.book.title_style.y_dis;
                push_text_ops(
                    ops,
                    &font_id,
                    &ctx.book.pager_style.color,
                    ctx.book.pager_style.font_size,
                    x,
                    y,
                    0.0,
                    &ch.to_string(),
                );
            }
        }
    }
}

fn draw_line(ops: &mut Vec<Op>, line: &LineSpec) {
    if line.wavy {
        draw_wavy_line(ops, line);
        return;
    }
    let mut points = Vec::with_capacity(2);
    points.push(LinePoint {
        p: Point::new(px_to_mm(line.x1), px_to_mm(line.y1)),
        bezier: false,
    });
    points.push(LinePoint {
        p: Point::new(px_to_mm(line.x2), px_to_mm(line.y2)),
        bezier: false,
    });
    ops.push(Op::SetOutlineColor {
        col: pdf_color(&line.color),
    });
    ops.push(Op::SetOutlineThickness {
        pt: px_to_pt(line.width),
    });
    ops.push(Op::DrawLine {
        line: Line {
            points,
            is_closed: false,
        },
    });
}

fn draw_wavy_line(ops: &mut Vec<Op>, line: &LineSpec) {
    let segments = ((line.y2 - line.y1).abs().max(20.0) / 12.0).ceil() as usize;
    let amplitude = (line.y2 - line.y1).abs().max(1.0) * 0.05;
    let wavelength = (line.y2 - line.y1).abs().max(1.0) / (segments as f32);
    let mut points = Vec::with_capacity(segments + 1);
    for i in 0..=segments {
        let t = i as f32 / segments as f32;
        let y = line.y1 + (line.y2 - line.y1) * t;
        let wave = amplitude * (2.0 * std::f32::consts::PI * (y - line.y1) / wavelength).sin();
        let x = line.x1 + wave;
        points.push(LinePoint {
            p: Point::new(px_to_mm(x), px_to_mm(y)),
            bezier: false,
        });
    }
    ops.push(Op::SetOutlineColor {
        col: pdf_color(&line.color),
    });
    ops.push(Op::SetOutlineThickness {
        pt: px_to_pt(line.width),
    });
    ops.push(Op::DrawLine {
        line: Line {
            points,
            is_closed: false,
        },
    });
}

fn draw_glyph(ops: &mut Vec<Op>, font_ids: &[Option<FontId>], glyph: &GlyphSpec) -> Result<()> {
    if let Some(font_id) = font_id(font_ids, glyph.font_idx) {
        push_text_ops(
            ops,
            &font_id,
            &glyph.color,
            glyph.font_size,
            glyph.x,
            glyph.y,
            glyph.rotate_deg,
            &glyph.ch.to_string(),
        );
    }
    Ok(())
}

fn push_text_ops(
    ops: &mut Vec<Op>,
    font_id: &FontId,
    color: &RgbColor,
    font_size: f32,
    x: f32,
    y: f32,
    rotate_deg: f32,
    text: &str,
) {
    ops.push(Op::StartTextSection);
    ops.push(Op::SetFillColor {
        col: pdf_color(color),
    });
    ops.push(Op::SetFontSize {
        size: Pt(font_size),
        font: font_id.clone(),
    });
    if rotate_deg.abs() > f32::EPSILON {
        ops.push(Op::SetTextMatrix {
            matrix: TextMatrix::TranslateRotate(px_to_mm(x).into(), px_to_mm(y).into(), rotate_deg),
        });
    } else {
        ops.push(Op::SetTextCursor {
            pos: Point::new(px_to_mm(x), px_to_mm(y)),
        });
    }
    ops.push(Op::WriteText {
        font: font_id.clone(),
        items: vec![TextItem::Text(text.to_string())],
    });
    ops.push(Op::EndTextSection);
}

fn font_id(fonts: &[Option<FontId>], idx: usize) -> Option<FontId> {
    if idx == 0 {
        return None;
    }
    fonts.get(idx - 1).and_then(|id| id.clone())
}

fn px_to_mm(value: f32) -> Mm {
    Mm(value * PX_TO_MM)
}

fn px_to_pt(value: f32) -> Pt {
    let mm = px_to_mm(value);
    mm.into()
}

fn pdf_color(color: &RgbColor) -> Color {
    Color::Rgb(Rgb::new(color.r, color.g, color.b, None))
}

fn register_image(doc: &mut PdfDocument, image: &DynamicImage) -> Result<XObjectId> {
    let raw = raw_image_from_dynamic(image);
    Ok(doc.add_image(&raw))
}

fn raw_image_from_dynamic(image: &DynamicImage) -> RawImage {
    let rgba = image.to_rgba8();
    let width = rgba.width() as usize;
    let height = rgba.height() as usize;
    let pixels = rgba.into_raw();
    RawImage {
        pixels: RawImageData::U8(pixels),
        width,
        height,
        data_format: RawImageFormat::RGBA8,
        tag: Vec::new(),
    }
}

#[derive(Debug, Clone)]
struct StampSpec {
    page: usize,
    col_begin: usize,
    row_begin: usize,
    cols: usize,
    path: PathBuf,
}

fn load_stamps(_ctx: &RenderContext, output_path: &Path) -> Result<HashMap<usize, Vec<StampSpec>>> {
    let stamps_cfg = output_path
        .parent()
        .map(|p| p.join("yins.cfg"))
        .unwrap_or_else(|| PathBuf::from("yins.cfg"));
    if !stamps_cfg.exists() {
        return Ok(HashMap::new());
    }
    let content = std::fs::read_to_string(&stamps_cfg)
        .with_context(|| format!("读取印章配置失败: {}", stamps_cfg.display()))?;
    let stem = output_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or_default()
        .to_string();
    let mut map: HashMap<usize, Vec<StampSpec>> = HashMap::new();
    for (lineno, line) in content.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let mut parts = trimmed.split('|');
        let pdf_name = parts.next().unwrap_or("").trim();
        let pos = parts.next().unwrap_or("").trim();
        let file = parts.next().unwrap_or("").trim();
        if pdf_name != stem && pdf_name != "*" {
            continue;
        }
        let parsed = pos
            .split(',')
            .filter_map(|x| x.trim().parse::<usize>().ok())
            .collect::<Vec<_>>();
        if parsed.len() != 4 || file.is_empty() {
            eprintln!(
                "忽略 yins.cfg 第 {} 行：格式错误 (需要 pdf|page,col,row,cols|file)",
                lineno + 1
            );
            continue;
        }
        let spec = StampSpec {
            page: parsed[0],
            col_begin: parsed[1],
            row_begin: parsed[2],
            cols: parsed[3],
            path: stamps_cfg
                .parent()
                .unwrap_or_else(|| Path::new("."))
                .join("yins")
                .join(file),
        };
        map.entry(spec.page).or_default().push(spec);
    }
    Ok(map)
}

fn add_stamp(
    ops: &mut Vec<Op>,
    stamp: &StampSpec,
    ctx: &RenderContext,
    doc: &mut PdfDocument,
) -> Result<()> {
    if !stamp.path.exists() {
        eprintln!("印章文件不存在，跳过: {}", stamp.path.display());
        return Ok(());
    }
    let image = image::open(&stamp.path)
        .with_context(|| format!("读取印章图片失败: {}", stamp.path.display()))?;
    let raw = raw_image_from_dynamic(&image);
    let id = doc.add_image(&raw);

    let cw = (ctx.canvas.canvas_width
        - ctx.canvas.margins_left
        - ctx.canvas.margins_right
        - ctx.canvas.leaf_center_width)
        / ctx.canvas.leaf_col as f32;
    let rh = (ctx.canvas.canvas_height - ctx.canvas.margins_top - ctx.canvas.margins_bottom)
        / ctx.book.row_num as f32;

    let mut x = ctx.canvas.canvas_width - ctx.canvas.margins_right - cw * stamp.col_begin as f32;
    if stamp.col_begin > ctx.canvas.leaf_col / 2 {
        x -= ctx.canvas.leaf_center_width;
    }
    let y = ctx.canvas.margins_bottom + rh * (stamp.row_begin.saturating_sub(1)) as f32;
    let target_w = cw * stamp.cols as f32;

    let source_w_pt = Px(raw.width).into_pt(IMAGE_DPI).0;
    let source_h_pt = Px(raw.height).into_pt(IMAGE_DPI).0;
    let target_w_pt = px_to_pt(target_w).0;
    let scale = if source_w_pt > 0.0 {
        target_w_pt / source_w_pt
    } else {
        1.0
    };
    let scale_y = if source_h_pt > 0.0 { scale } else { 1.0 };

    ops.push(Op::UseXobject {
        id,
        transform: XObjectTransform {
            translate_x: Some(px_to_mm(x).into()),
            translate_y: Some(px_to_mm(y).into()),
            scale_x: Some(scale),
            scale_y: Some(scale_y),
            dpi: Some(IMAGE_DPI),
            ..Default::default()
        },
    });
    Ok(())
}

fn build_outline_map(
    plan: &DocumentPlan,
    ctx: &RenderContext,
) -> Option<HashMap<usize, Vec<String>>> {
    if !ctx.book.title_style.directory {
        return None;
    }
    let mut map: HashMap<usize, Vec<String>> = HashMap::new();
    for outline in &plan.outlines {
        map.entry(outline.page_number)
            .or_default()
            .push(outline.title.clone());
    }
    Some(map)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{BookConfig, CanvasConfig};
    use crate::fonts::FontManager;
    use crate::numerals::NumeralMap;
    use crate::plan::{CoverPlan, DocumentPlan, GlyphSpec, OutlineEntry, PagePlan};
    use std::path::Path;

    #[test]
    fn render_document_produces_pdf_with_bookmark() {
        let book = BookConfig::load("books/01/book.cfg").expect("load sample book configuration");
        let canvas_path = format!("canvas/{}.cfg", book.canvas_id);
        let canvas =
            CanvasConfig::load(&canvas_path).expect("load canvas configuration referenced by book");
        let fonts =
            FontManager::new(&book, Path::new("fonts")).expect("load fonts for renderer tests");
        let numerals =
            NumeralMap::load("db/num2zh_jid.txt").expect("load numeral mapping for tests");

        let sample_font_idx = book
            .fonts
            .text_stack
            .first()
            .copied()
            .expect("book config must define a text font stack");

        let glyph = GlyphSpec {
            ch: '測',
            font_idx: sample_font_idx,
            font_size: 48.0,
            x: 100.0,
            y: 100.0,
            rotate_deg: 0.0,
            color: book.text_font_color,
        };

        let page = PagePlan {
            number: 1,
            title: "测试页面".into(),
            glyphs: vec![glyph],
            lines: Vec::new(),
        };
        let plan = DocumentPlan {
            cover: CoverPlan::Generated,
            cover_path: None,
            pages: vec![page],
            outlines: vec![OutlineEntry {
                title: "卷一".into(),
                page_number: 1,
            }],
        };

        let ctx = RenderContext {
            book: &book,
            canvas: &canvas,
            fonts: &fonts,
            numerals: &numerals,
            background: Some(DynamicImage::new_rgba8(16, 16)),
            cover_image: None,
        };

        let output_path = std::env::temp_dir().join("vrain_renderer_smoke.pdf");
        render_document(&plan, &ctx, &output_path).expect("render minimal pdf");

        let bytes = std::fs::read(&output_path).expect("read rendered pdf");
        assert!(
            bytes.len() > 0,
            "rendered pdf must contain at least some bytes"
        );

        let mut warnings = Vec::new();
        let parsed = printpdf::PdfDocument::parse(
            &bytes,
            &printpdf::PdfParseOptions::default(),
            &mut warnings,
        )
        .expect("parse rendered pdf");
        assert!(
            !parsed.bookmarks.map.is_empty(),
            "expected at least one bookmark generated from outlines",
        );
        let _ = std::fs::remove_file(output_path);
    }
}

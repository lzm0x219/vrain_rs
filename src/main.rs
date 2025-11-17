mod args;
mod color;
mod config;
mod fonts;
mod layout;
mod layout_engine;
mod background;
mod multirows;
mod numerals;
mod plan;
mod preprocess;
mod renderer;
mod typesetter;

use anyhow::{Result, anyhow, bail, Context};
use args::Cli;
use clap::Parser;
use config::{BookConfig, CanvasConfig};
use fonts::FontManager;
use layout::Layout;
use multirows::MultiRowsMode;
use numerals::NumeralMap;
use plan::TypesetOptions;
use preprocess::load_corpus;
use image::{self as pdf_image, DynamicImage};
use renderer::{RenderContext, render_document};
use std::path::{Path, PathBuf};
use typesetter::Typesetter;

fn main() -> Result<()> {
    let cli = Cli::parse();
    let to = cli.to.unwrap_or(cli.from);
    if to < cli.from {
        bail!("--to must be >= --from");
    }

    let book_dir = cli.books_root.join(&cli.book_id);
    let text_dir = book_dir.join("text");
    ensure_exists(&book_dir, "book directory")?;
    ensure_exists(&text_dir, "book text directory")?;

    let book_cfg_path = book_dir.join("book.cfg");
    ensure_exists(&book_cfg_path, "book configuration")?;
    let book_cfg = BookConfig::load(&book_cfg_path)?;
    book_cfg.validate()?;
    let canvas_cfg_path = cli.canvas_root.join(format!("{}.cfg", book_cfg.canvas_id));
    ensure_exists(&canvas_cfg_path, "canvas configuration")?;
    let canvas_cfg = CanvasConfig::load(&canvas_cfg_path)?;
    canvas_cfg.validate()?;
    println!("Loaded '{}' by {}", book_cfg.title, book_cfg.author);

    if cli.generate_bg {
        let out_path = cli
            .bg_output
            .clone()
            .unwrap_or_else(|| cli.canvas_root.join(format!("{}.jpg", book_cfg.canvas_id)));
        if let Some(parent) = out_path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("create background output dir {}", parent.display()))?;
        }
        let image = background::generate_bamboo_background(&canvas_cfg);
        image
            .save(&out_path)
            .with_context(|| format!("write generated background {}", out_path.display()))?;
        println!("Generated background saved to {}", out_path.display());
        return Ok(());
    }

    let multirows_mode = MultiRowsMode::from_flags(
        canvas_cfg.multirows_enabled,
        canvas_cfg.multirows_count,
        book_cfg.multirows_horizontal_layout,
    );

    let layout = Layout::build(&book_cfg, &canvas_cfg, multirows_mode)?;
    let fonts = FontManager::new(&book_cfg, &cli.fonts_root)?;
    let numerals = NumeralMap::load(cli.db_root.join("num2zh_jid.txt"))?;
    println!(
        "Layout: {} columns x {} rows ({} glyphs/page)",
        canvas_cfg.leaf_col, book_cfg.row_num, layout.per_page
    );

    let corpus = load_corpus(&book_dir, &book_cfg)?;

    let background_candidates = vec![
        cli.canvas_root.join(format!("{}.jpg", book_cfg.canvas_id)),
        cli.canvas_root.join(format!("{}.png", book_cfg.canvas_id)),
    ];
    let (_bg_path, mut background_image) = load_first_available_image(&background_candidates);
    if background_image.is_none() {
        background_image = Some(background::generate_bamboo_background(&canvas_cfg));
    }

    let cover_candidates = vec![book_dir.join("cover.jpg"), book_dir.join("cover.png")];
    let (cover_plan_path, cover_image) = load_first_available_image(&cover_candidates);

    let typeset_opts = TypesetOptions {
        from: cli.from,
        to,
        test_pages: cli.test_pages,
        verbose: cli.verbose,
        cover_image: cover_plan_path.clone(),
    };

    let mut typesetter =
        Typesetter::new(&book_cfg, &layout, &fonts, &numerals, &corpus, typeset_opts)?;
    let plan = typesetter.build_plan()?;
    plan.validate()?;
    if let Some(path) = &cli.debug_plan {
        if let Err(err) = plan.write_debug_json(path) {
            eprintln!(
                "Failed to write plan debug JSON ({}): {}",
                path.display(),
                err
            );
        } else {
            println!("Document plan debug JSON written to {}", path.display());
        }
    }

    let render_ctx = RenderContext {
        book: &book_cfg,
        canvas: &canvas_cfg,
        fonts: &fonts,
        numerals: &numerals,
        background: background_image,
        cover_image,
    };
    let output_name = format!("《{}》文本{}至{}.pdf", book_cfg.title, cli.from, to);
    let output_path = book_dir.join(&output_name);
    println!("Rendering PDF to {}", output_path.display());
    render_document(&plan, &render_ctx, &output_path)?;

    if cli.compress {
        if let Err(err) = compress_pdf(&output_path) {
            eprintln!("PDF compression failed: {err}");
        }
    }

    println!("Done.");
    Ok(())
}

fn ensure_exists(path: &Path, label: &str) -> Result<()> {
    if !path.exists() {
        bail!("{} not found: {}", label, path.display());
    }
    Ok(())
}

fn compress_pdf(output: &Path) -> Result<()> {
    // require gs to be present
    if std::process::Command::new("which")
        .arg("gs")
        .status()
        .map(|s| !s.success())
        .unwrap_or(true)
    {
        println!("Ghostscript not found, skip compression. Install gs to enable -c.");
        return Ok(());
    }
    let compressed = output.with_file_name(format!(
        "{}_compressed.pdf",
        output
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("output")
    ));
    let status = std::process::Command::new("gs")
        .args([
            "-sDEVICE=pdfwrite",
            "-dCompatibilityLevel=1.4",
            "-dPDFSETTINGS=/screen",
            "-dNOPAUSE",
            "-dQUIET",
            "-dBATCH",
        ])
        .arg(format!("-sOutputFile={}", compressed.display()))
        .arg(output)
        .status();
    match status {
        Ok(code) if code.success() => {
            println!("Compressed PDF saved to {}", compressed.display());
            Ok(())
        }
        Ok(code) => Err(anyhow!("ghostscript exited with {}", code)),
        Err(err) => Err(anyhow!("ghostscript invocation failed: {err}")),
    }
}

fn load_first_available_image(paths: &[PathBuf]) -> (Option<PathBuf>, Option<DynamicImage>) {
    for path in paths {
        if path.exists() {
            match pdf_image::open(path) {
                Ok(img) => return (Some(path.clone()), Some(img)),
                Err(_) => continue,
            }
        }
    }
    (None, None)
}

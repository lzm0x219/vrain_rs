use std::path::PathBuf;

use clap::Parser;

#[derive(Debug, Parser)]
#[command(author = "vRain Project", version)]
#[command(about = "Experimental Rust port of the vRain typesetting tool")]
pub struct Cli {
    /// Book identifier (maps to books/<book>/)
    #[arg(short = 'b', long = "book", value_name = "BOOK_ID")]
    pub book_id: String,

    /// 仅生成背景图并退出
    #[arg(long = "generate-bg")]
    pub generate_bg: bool,

    /// 背景图输出路径（默认：canvas/<canvas_id>.jpg）
    #[arg(long = "bg-output", value_name = "BG_PATH")]
    pub bg_output: Option<PathBuf>,

    /// Start chapter/text index (matches NN?.txt). Default: 1
    #[arg(short = 'f', long = "from", value_name = "START", default_value_t = 1)]
    pub from: usize,

    /// End chapter/text index (inclusive). If unset, process only START.
    #[arg(short = 't', long = "to", value_name = "END")]
    pub to: Option<usize>,

    /// Limit number of pages generated for inspection (test mode)
    #[arg(short = 'z', long = "test-pages", value_name = "NUM")]
    pub test_pages: Option<usize>,

    /// Books directory (holds book_id/book.cfg and text/)
    #[arg(long = "books-dir", value_name = "PATH", default_value = "books")]
    pub books_root: PathBuf,

    /// Canvas directory (holds {canvas_id}.cfg)
    #[arg(long = "canvas-dir", value_name = "PATH", default_value = "canvas")]
    pub canvas_root: PathBuf,

    /// Fonts directory
    #[arg(long = "fonts-dir", value_name = "PATH", default_value = "fonts")]
    pub fonts_root: PathBuf,

    /// Database directory (contains num2zh_jid.txt)
    #[arg(long = "db-dir", value_name = "PATH", default_value = "db")]
    pub db_root: PathBuf,

    /// Compress PDF via Ghostscript after generation (macOS only, matches -c)
    #[arg(short = 'c', long = "compress")]
    pub compress: bool,

    /// Verbose glyph logging (matches Perl -v)
    #[arg(short = 'v', long = "verbose")]
    pub verbose: bool,

    /// Export the computed DocumentPlan as JSON for debugging
    #[arg(long = "debug-plan", value_name = "JSON_PATH")]
    pub debug_plan: Option<PathBuf>,
}

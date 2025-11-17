#![allow(dead_code)]

use crate::config::BookConfig;
use crate::fonts::FontManager;
use crate::layout::Layout;
use crate::layout_engine::LayoutEngine;
use crate::numerals::NumeralMap;
use crate::plan::{CoverPlan, DocumentPlan, OutlineEntry, PagePlan, TypesetOptions};
use crate::preprocess::TextCorpus;
use anyhow::Result;
use std::mem;

pub struct Typesetter<'a> {
    book: &'a BookConfig,
    layout: &'a Layout,
    fonts: &'a FontManager,
    numerals: &'a NumeralMap,
    corpus: &'a TextCorpus,
    options: TypesetOptions,
}

impl<'a> Typesetter<'a> {
    pub fn new(
        book: &'a BookConfig,
        layout: &'a Layout,
        fonts: &'a FontManager,
        numerals: &'a NumeralMap,
        corpus: &'a TextCorpus,
        options: TypesetOptions,
    ) -> Result<Self> {
        Ok(Self {
            book,
            layout,
            fonts,
            numerals,
            corpus,
            options,
        })
    }

    pub fn build_plan(&mut self) -> Result<DocumentPlan> {
        let (cover_plan, cover_path) = match &self.options.cover_image {
            Some(path) => (CoverPlan::Image, Some(path.clone())),
            None => (CoverPlan::Generated, None),
        };

        let mut pages: Vec<PagePlan> = Vec::new();
        let mut outlines: Vec<OutlineEntry> = Vec::new();
        let mut current_page = PagePlan {
            number: 1,
            title: String::new(),
            glyphs: Vec::new(),
            lines: Vec::new(),
        };
        let mut pcnt: usize = 0;
        let mut next_page_number = 1usize;
        let mut generated_pages = 0usize;
        let mut bookline_active = false;

        let engine = LayoutEngine {
            book: self.book,
            layout: self.layout,
            fonts: self.fonts,
            options: &self.options,
        };

        for idx in self.options.from..=self.options.to {
            let entry = self.corpus.entry(idx)?;
            let title_text = self.compute_entry_title(idx);

            if !current_page.glyphs.is_empty() {
                let finished_page = mem::replace(
                    &mut current_page,
                    PagePlan {
                        number: next_page_number + 1,
                        title: title_text.clone(),
                        glyphs: Vec::new(),
                        lines: Vec::new(),
                    },
                );
                pages.push(finished_page);
                generated_pages += 1;
                if self.reached_limit(generated_pages) {
                    break;
                }
                next_page_number += 1;
                pcnt = 0;
            } else {
                current_page.title = title_text.clone();
            }

            outlines.push(OutlineEntry {
                title: title_text.clone(),
                page_number: next_page_number,
            });

            engine.process_entry(
                &entry.data,
                &title_text,
                &mut current_page,
                &mut pages,
                &mut pcnt,
                &mut generated_pages,
                &mut next_page_number,
                &mut bookline_active,
            )?;

            if self.reached_limit(generated_pages) {
                break;
            }
        }

        if !current_page.glyphs.is_empty() && !self.reached_limit(generated_pages) {
            pages.push(current_page);
        }

        Ok(DocumentPlan {
            cover: cover_plan,
            cover_path,
            pages,
            outlines,
        })
    }

    fn compute_entry_title(&self, idx: usize) -> String {
        let mut chars: Vec<char> = self.book.title.chars().collect();
        if let Some(mut postfix) = self.book.title_style.postfix.clone() {
            let mut cid = idx;
            if self.corpus.has_text000 {
                cid = cid.saturating_sub(1);
            }
            if cid == 0 {
                postfix = "序".into();
            } else if self.corpus.has_text999 && idx == self.corpus.total_entries() {
                postfix = "附".into();
            } else if postfix.contains('X') {
                let zh = self.numerals.render(cid);
                postfix = postfix.replace('X', &zh);
            }
            chars.extend(postfix.chars());
        }
        chars.into_iter().collect()
    }

    fn reached_limit(&self, generated_pages: usize) -> bool {
        self.options
            .test_pages
            .map(|limit| generated_pages >= limit)
            .unwrap_or(false)
    }
}

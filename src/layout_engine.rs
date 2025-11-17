use crate::config::BookConfig;
use crate::fonts::{FontManager, FontPick};
use crate::layout::{Cell, Layout};
use crate::plan::{GlyphSpec, LineSpec, PagePlan, TypesetOptions};
use anyhow::{Result, anyhow};
use zhconv::{Variant, zhconv};

use std::collections::VecDeque;
use std::iter::Peekable;
use std::str::Chars;

pub struct LayoutEngine<'a> {
    pub book: &'a BookConfig,
    pub layout: &'a Layout,
    pub fonts: &'a FontManager,
    pub options: &'a TypesetOptions,
}

impl<'a> LayoutEngine<'a> {
    pub fn process_entry(
        &self,
        entry: &str,
        title_text: &str,
        mut current_page: &mut PagePlan,
        pages: &mut Vec<PagePlan>,
        pcnt: &mut usize,
        generated_pages: &mut usize,
        next_page_number: &mut usize,
        bookline_active: &mut bool,
    ) -> Result<()> {
        let pos_l = &self.layout.pos_l;
        let pos_left = |idx: usize| pos_l.get(idx).copied();
        let mut chars = entry.chars().peekable();
        let mut last_pos: Option<Cell> = None;
        let mut comment_queue: Vec<char> = Vec::new();

        while let Some(ch) = chars.next() {
            if ch == '\n' || ch == '\r' {
                continue;
            }
            match ch {
                '%' => {
                    self.skip_row_padding(&mut chars);
                    self.finalize_page(
                        &mut current_page,
                        pages,
                        pcnt,
                        generated_pages,
                        next_page_number,
                        title_text,
                    );
                    last_pos = None;
                    if self.reached_limit(*generated_pages) {
                        break;
                    }
                    continue;
                }
                '$' => {
                    self.skip_row_padding(&mut chars);
                    let half = self.layout.per_page / 2;
                    if *pcnt == 0 || *pcnt == half {
                        continue;
                    }
                    if *pcnt < half {
                        *pcnt = half;
                    } else {
                        *pcnt = self.layout.per_page;
                    }
                    continue;
                }
                '^' => {
                    if self.layout.multirows_bands > 1 {
                        let band = self.layout.columns * self.layout.rows_per_column;
                        let next_band_start = ((*pcnt) / band + 1) * band;
                        if next_band_start < self.layout.per_page {
                            *pcnt = next_band_start;
                        } else {
                            *pcnt = self.layout.per_page;
                        }
                        last_pos = None;
                        continue;
                    }
                }
                '&' => {
                    self.skip_row_padding(&mut chars);
                    let rows_per_column = self.layout.rows_per_column;
                    if rows_per_column == 0 {
                        continue;
                    }
                    let last_col_start = self.layout.per_page.saturating_sub(rows_per_column);
                    let threshold = last_col_start.saturating_add(1);
                    if *pcnt <= threshold {
                        *pcnt = last_col_start;
                    }
                    continue;
                }
                '《' => {
                    *bookline_active = true;
                    if self.book.book_line_flag {
                        continue;
                    }
                }
                '》' => {
                    *bookline_active = false;
                    if self.book.book_line_flag {
                        continue;
                    }
                }
                '【' => {
                    while let Some(next) = chars.next() {
                        if next == '】' {
                            break;
                        }
                        comment_queue.push(next);
                    }
                    if !comment_queue.is_empty() {
                        self.render_comments(
                            &mut current_page,
                            pages,
                            pcnt,
                            generated_pages,
                            next_page_number,
                            &mut comment_queue,
                            title_text,
                        )?;
                        last_pos = None;
                    }
                    continue;
                }
                _ => {}
            }

            let is_nop = self.book.punctuation.text_nop.chars.contains(&ch);
            let is_rot = self.book.punctuation.text_rotate.chars.contains(&ch);
            let consumes_slot = !is_nop;

            if consumes_slot && *pcnt == self.layout.per_page {
                self.finalize_page(
                    &mut current_page,
                    pages,
                    pcnt,
                    generated_pages,
                    next_page_number,
                    title_text,
                );
                last_pos = None;
                if self.reached_limit(*generated_pages) {
                    break;
                }
            }

            if is_rot {
                *pcnt += 1;
                let pos =
                    pos_left(*pcnt).ok_or_else(|| anyhow!("layout index {} out of range", pcnt))?;
                if let Some(glyph) = self.build_text_glyph(pos, ch, false, false, true) {
                    current_page.glyphs.push(glyph);
                    last_pos = Some(pos);
                    if *bookline_active && self.book.book_line_flag && ch != ' ' {
                        if let Some(bline) = &self.book.bookline {
                            current_page.lines.push(LineSpec {
                                x1: pos.x - bline.width,
                                x2: pos.x - bline.width,
                                y1: pos.y - self.layout.rh * 0.3,
                                y2: pos.y + self.layout.rh * 0.7,
                                width: bline.width,
                                color: bline.color,
                                wavy: true,
                            });
                        }
                    }
                }
                continue;
            }

            if is_nop {
                let pos_prev = last_pos
                    .or_else(|| pos_left((*pcnt).max(1)))
                    .unwrap_or(Cell { x: 0.0, y: 0.0 });
                if let Some(glyph) = self.build_text_glyph(pos_prev, ch, false, true, false) {
                    current_page.glyphs.push(glyph);
                }
                continue;
            }

            *pcnt += 1;
            let pos =
                pos_left(*pcnt).ok_or_else(|| anyhow!("layout index {} out of range", pcnt))?;

            if let Some(glyph) = self.build_text_glyph(pos, ch, false, false, false) {
                if self.options.verbose {
                    println!(
                        "[page {} slot {}] char '{}'",
                        current_page.number, pcnt, glyph.ch
                    );
                }
                current_page.glyphs.push(glyph);
                last_pos = Some(pos);
                if *bookline_active && self.book.book_line_flag && ch != ' ' {
                    if let Some(bline) = &self.book.bookline {
                        current_page.lines.push(LineSpec {
                            x1: pos.x - bline.width,
                            x2: pos.x - bline.width,
                            y1: pos.y - self.layout.rh * 0.3,
                            y2: pos.y + self.layout.rh * 0.7,
                            width: bline.width,
                            color: bline.color,
                            wavy: true,
                        });
                    }
                }
            }

            if *pcnt == self.layout.per_page {
                if let Some(&next) = chars.peek() {
                    if self.book.punctuation.text_nop.chars.contains(&next) {
                        chars.next();
                        let pos_prev = last_pos.unwrap_or(pos);
                        if let Some(pglyph) =
                            self.build_text_glyph(pos_prev, next, false, true, false)
                        {
                            current_page.glyphs.push(pglyph);
                        }
                    }
                }
            }
        }
        Ok(())
    }

    fn render_comments(
        &self,
        current_page: &mut PagePlan,
        pages: &mut Vec<PagePlan>,
        pcnt: &mut usize,
        generated_pages: &mut usize,
        next_page_number: &mut usize,
        queue: &mut Vec<char>,
        title_text: &str,
    ) -> Result<()> {
        if queue.is_empty() {
            return Ok(());
        }
        let mut remaining: VecDeque<char> = queue.drain(..).collect();
        let mut comment_bookline_active = false;
        let mut comment_last_slot: Option<Cell> = None;

        'outer: while let Some(_) = remaining.front() {
            if *pcnt >= self.layout.per_page {
                self.finalize_page(
                    current_page,
                    pages,
                    pcnt,
                    generated_pages,
                    next_page_number,
                    title_text,
                );
                comment_last_slot = None;
                continue 'outer;
            }
            let rows_per_column = self.layout.rows_per_column;
            let row_idx = (*pcnt % rows_per_column) + 1;
            let slots_in_column = rows_per_column.saturating_sub(row_idx - 1);
            if slots_in_column == 0 {
                self.finalize_page(
                    current_page,
                    pages,
                    pcnt,
                    generated_pages,
                    next_page_number,
                    title_text,
                );
                comment_last_slot = None;
                continue 'outer;
            }

            let slot_needed = self.count_comment_slots(&remaining);
            if slot_needed == 0 {
                while let Some(ch) = remaining.pop_front() {
                    if self.should_skip_bookline_char(ch) {
                        continue;
                    }
                    if let Some(last) = comment_last_slot {
                        if self.book.punctuation.comment_nop.chars.contains(&ch) {
                            if let Some(spec) = self.build_text_glyph(last, ch, true, true, false) {
                                current_page.glyphs.push(spec);
                            }
                        }
                    }
                }
                break;
            }
            let take_pairs = slots_in_column.min(slot_needed);
            if *pcnt + take_pairs > self.layout.per_page {
                self.finalize_page(
                    current_page,
                    pages,
                    pcnt,
                    generated_pages,
                    next_page_number,
                    title_text,
                );
                comment_last_slot = None;
                continue 'outer;
            }

            let mut local_chars = Vec::new();
            let mut needed_chars = take_pairs * 2;
            while let Some(ch) = remaining.pop_front() {
                if self.comment_char_consumes_slot(ch) {
                    needed_chars = needed_chars.saturating_sub(1);
                }
                local_chars.push(ch);
                if needed_chars == 0 || remaining.is_empty() {
                    break;
                }
            }

            let mut positions: Vec<Cell> = Vec::with_capacity(take_pairs * 2);
            for offset in 1..=take_pairs {
                if let Some(pos) = self.layout.pos_right(*pcnt + offset) {
                    positions.push(*pos);
                } else {
                    self.finalize_page(
                        current_page,
                        pages,
                        pcnt,
                        generated_pages,
                        next_page_number,
                        title_text,
                    );
                    comment_last_slot = None;
                    for ch in local_chars.into_iter().rev() {
                        remaining.push_front(ch);
                    }
                    continue 'outer;
                }
            }
            for offset in 1..=take_pairs {
                if let Some(pos) = self.layout.pos_left(*pcnt + offset) {
                    positions.push(*pos);
                } else {
                    self.finalize_page(
                        current_page,
                        pages,
                        pcnt,
                        generated_pages,
                        next_page_number,
                        title_text,
                    );
                    comment_last_slot = None;
                    for ch in local_chars.into_iter().rev() {
                        remaining.push_front(ch);
                    }
                    continue 'outer;
                }
            }

            let mut pos_iter = positions.into_iter();
            let mut last_pos = comment_last_slot;
            let mut non_nop_count = 0usize;

            for idx in 0..local_chars.len() {
                let ch = local_chars[idx];
                if self.book.book_line_flag {
                    if ch == '《' {
                        comment_bookline_active = true;
                        continue;
                    } else if ch == '》' {
                        comment_bookline_active = false;
                        continue;
                    }
                }
                let is_rot = self.book.punctuation.comment_rotate.chars.contains(&ch);
                let is_nop = self.book.punctuation.comment_nop.chars.contains(&ch);
                if is_nop {
                    if let Some(last) = last_pos {
                        if let Some(spec) = self.build_text_glyph(last, ch, true, true, false) {
                            current_page.glyphs.push(spec);
                        }
                    }
                    continue;
                }
                let pos = match pos_iter.next() {
                    Some(p) => p,
                    None => {
                        remaining.push_front(ch);
                        for rest in local_chars[idx + 1..].iter().rev() {
                            remaining.push_front(*rest);
                        }
                        continue 'outer;
                    }
                };
                last_pos = Some(pos);
                comment_last_slot = Some(pos);
                non_nop_count += 1;
                if is_rot {
                    if let Some(spec) = self.build_text_glyph(pos, ch, true, false, true) {
                        current_page.glyphs.push(spec);
                    }
                } else if let Some(spec) = self.build_text_glyph(pos, ch, true, false, false) {
                    current_page.glyphs.push(spec);
                }
                if comment_bookline_active && self.book.book_line_flag && ch != ' ' {
                    if let Some(bline) = &self.book.bookline {
                        current_page.lines.push(LineSpec {
                            x1: pos.x - bline.width,
                            x2: pos.x - bline.width,
                            y1: pos.y - self.layout.rh * 0.3,
                            y2: pos.y + self.layout.rh * 0.7,
                            width: bline.width,
                            color: bline.color,
                            wavy: true,
                        });
                    }
                }
            }

            if non_nop_count == 0 {
                continue 'outer;
            }
            let slots_used = (non_nop_count + 1) / 2;
            *pcnt += slots_used;
        }

        if !remaining.is_empty() {
            queue.extend(remaining.into_iter());
        }
        Ok(())
    }

    fn build_text_glyph(
        &self,
        pos: Cell,
        ch: char,
        is_comment: bool,
        is_nop: bool,
        is_rot: bool,
    ) -> Option<GlyphSpec> {
        let stack = if is_comment {
            &self.fonts.comment_stack
        } else {
            &self.fonts.text_stack
        };
        let (mut ch, mut pick) = self.pick_with_try_st(ch, stack);
        if pick.is_none() {
            ch = '□';
            pick = self.fonts.pick_font(ch, stack);
        }
        pick.map(|font_pick| {
            let mut font_size = if is_comment {
                font_pick.font.slot.comment_size
            } else {
                font_pick.font.slot.text_size
            };
            let width = if is_comment {
                self.layout.cw / 2.0
            } else {
                self.layout.cw
            };

            let mut fx = pos.x;
            let mut fy = pos.y;
            let mut color = if is_comment {
                self.book.comment_font_color
            } else {
                self.book.text_font_color
            };
            if self.book.text_modes.only_period && ch == '。' {
                color = self.book.text_modes.only_period_color.unwrap_or(color);
            }
            let mut rotate_deg = font_pick.font.slot.rotate_deg;

            if !is_nop && !is_rot {
                fx += (width - font_size) / 2.0;
            }
            if is_comment {
                fy += (self.layout.rh - font_size) / 4.0;
            }

            if is_nop {
                let adj = if is_comment {
                    &self.book.punctuation.comment_nop
                } else {
                    &self.book.punctuation.text_nop
                };
                font_size *= adj.scale;
                let cw = if is_comment {
                    self.layout.cw / 2.0
                } else {
                    self.layout.cw
                };
                fx += cw * adj.offset_x;
                fy -= self.layout.rh * adj.offset_y;
            }

            if is_rot {
                let adj = if is_comment {
                    &self.book.punctuation.comment_rotate
                } else {
                    &self.book.punctuation.text_rotate
                };
                font_size *= adj.scale;
                let cw = if is_comment {
                    self.layout.cw / 2.0
                } else {
                    self.layout.cw
                };
                fx += cw * adj.offset_x;
                fy += self.layout.rh * adj.offset_y;
                rotate_deg = -90.0;
            }

            GlyphSpec {
                ch,
                font_idx: font_pick.slot_index,
                font_size,
                x: fx,
                y: fy,
                rotate_deg,
                color,
            }
        })
    }

    fn pick_with_try_st<'font>(
        &'font self,
        ch: char,
        stack: &[usize],
    ) -> (char, Option<FontPick<'font>>) {
        if let Some(pick) = self.fonts.pick_font(ch, stack) {
            return (ch, Some(pick));
        }
        if !self.book.try_st {
            return (ch, None);
        }
        if let Some((alt, pick)) = self.try_convert_char(ch, Variant::ZhHant, stack) {
            return (alt, Some(pick));
        }
        if let Some((alt, pick)) = self.try_convert_char(ch, Variant::ZhHans, stack) {
            return (alt, Some(pick));
        }
        (ch, None)
    }

    fn try_convert_char<'font>(
        &'font self,
        ch: char,
        variant: Variant,
        stack: &[usize],
    ) -> Option<(char, FontPick<'font>)> {
        let converted = zhconv(&ch.to_string(), variant);
        let mut chars = converted.chars();
        let candidate = chars.next()?;
        if candidate == ch {
            return None;
        }
        self.fonts
            .pick_font(candidate, stack)
            .map(|pick| (candidate, pick))
    }

    fn finalize_page(
        &self,
        current_page: &mut PagePlan,
        pages: &mut Vec<PagePlan>,
        pcnt: &mut usize,
        generated_pages: &mut usize,
        next_page_number: &mut usize,
        title_text: &str,
    ) {
        if !current_page.glyphs.is_empty() {
            pages.push(current_page.clone());
            *generated_pages += 1;
        }
        *pcnt = 0;
        *next_page_number += 1;
        current_page.glyphs.clear();
        current_page.number = *next_page_number;
        current_page.title = title_text.to_string();
    }

    fn skip_row_padding<'b>(&self, chars: &mut Peekable<Chars<'b>>) {
        if self.book.row_num == 0 {
            return;
        }
        for _ in 0..self.book.row_num.saturating_sub(1) {
            if chars.next().is_none() {
                break;
            }
        }
    }

    fn comment_char_consumes_slot(&self, ch: char) -> bool {
        if self.should_skip_bookline_char(ch) {
            return false;
        }
        !self.book.punctuation.comment_nop.chars.contains(&ch)
    }

    fn count_comment_slots(&self, queue: &VecDeque<char>) -> usize {
        let consuming = queue
            .iter()
            .copied()
            .filter(|ch| self.comment_char_consumes_slot(*ch))
            .count();
        (consuming + 1) / 2
    }

    fn should_skip_bookline_char(&self, ch: char) -> bool {
        self.book.book_line_flag && (ch == '《' || ch == '》')
    }

    fn reached_limit(&self, generated_pages: usize) -> bool {
        self.options
            .test_pages
            .map(|limit| generated_pages >= limit)
            .unwrap_or(false)
    }
}

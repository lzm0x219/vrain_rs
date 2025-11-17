#![allow(dead_code)]

use crate::config::{BookConfig, ReplacementRules, TextModes};
use anyhow::{Context, Result, anyhow};
use std::fs;
use std::path::Path;

#[derive(Debug, Clone)]
pub struct TextEntry {
    pub name: String,
    pub ordinal: usize,
    pub data: String,
}

#[derive(Debug)]
pub struct TextCorpus {
    pub entries: Vec<Option<TextEntry>>,
    pub has_text000: bool,
    pub has_text999: bool,
}

impl TextCorpus {
    pub fn entry(&self, idx: usize) -> Result<&TextEntry> {
        self.entries
            .get(idx)
            .and_then(|e| e.as_ref())
            .ok_or_else(|| anyhow!("text entry {} not available", idx))
    }

    pub fn total_entries(&self) -> usize {
        self.entries
            .iter()
            .enumerate()
            .rev()
            .find(|(_, entry)| entry.is_some())
            .map(|(idx, _)| idx)
            .unwrap_or(0)
    }
}

pub fn load_corpus(book_dir: &Path, book: &BookConfig) -> Result<TextCorpus> {
    let text_dir = book_dir.join("text");
    let mut entries: Vec<Option<TextEntry>> = vec![None; 1000];
    let mut has_text000 = false;
    let mut has_text999 = false;

    let mut files = fs::read_dir(&text_dir)
        .with_context(|| format!("reading {}", text_dir.display()))?
        .filter_map(|entry| entry.ok())
        .filter(|entry| {
            entry
                .path()
                .extension()
                .map(|ext| ext.eq_ignore_ascii_case("txt"))
                .unwrap_or(false)
        })
        .collect::<Vec<_>>();
    files.sort_by_key(|entry| entry.file_name());

    for entry in files.into_iter() {
        let file_name = entry.file_name().to_string_lossy().to_string();
        let lower = file_name.to_ascii_lowercase();
        let stem = lower.trim_end_matches(".txt");
        if stem.chars().all(|c| c == '0') {
            has_text000 = true;
        }
        if lower == "999.txt" {
            has_text999 = true;
        }
        let Some(ordinal) = stem.parse::<usize>().ok() else {
            continue;
        };
        if ordinal >= entries.len() {
            entries.resize(ordinal + 1, None);
        }
        let content =
            fs::read_to_string(entry.path()).with_context(|| entry.path().display().to_string())?;
        let processed = process_text(&content, book)?;
        entries[ordinal] = Some(TextEntry {
            name: file_name,
            ordinal,
            data: processed,
        });
    }

    if entries.iter().all(|e| e.is_none()) {
        return Err(anyhow!("no .txt files found under {}", text_dir.display()));
    }

    Ok(TextCorpus {
        entries,
        has_text000,
        has_text999,
    })
}

fn process_text(content: &str, book: &BookConfig) -> Result<String> {
    let mut result = String::new();
    for raw_line in content.lines() {
        let trimmed = raw_line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let mut current: String = trimmed.chars().filter(|c| !c.is_whitespace()).collect();
        if current.is_empty() {
            continue;
        }
        apply_replacements(&mut current, &book.replacements);
        apply_text_modes(&mut current, &book.text_modes);
        current = current.replace('@', " ");

        let tmp_original = current.clone();
        let mut working = current.clone();

        remove_chars(
            &mut working,
            &book.punctuation.text_nop.chars,
            &book.punctuation.comment_strip_chars,
        );
        if book.book_line_flag {
            working.retain(|ch| ch != '《' && ch != '》');
        }
        let annotation_extra = count_annotation_slots(&working);
        strip_annotations(&mut working);
        let total_chars = working.chars().count() + annotation_extra;
        let spaces = missing_spaces(total_chars, book.row_num);

        result.push_str(&tmp_original);
        if spaces > 0 && spaces < book.row_num {
            for _ in 0..spaces {
                result.push(' ');
            }
        }
    }
    Ok(result)
}

fn apply_replacements(text: &mut String, rules: &ReplacementRules) {
    for (from, to) in &rules.comma_pairs {
        *text = text.replace(*from, to);
    }
    for (from, to) in &rules.number_pairs {
        *text = text.replace(*from, to);
    }
    for token in &rules.delete_tokens {
        if token.is_empty() {
            continue;
        }
        *text = text.replace(token, "");
    }
}

fn apply_text_modes(text: &mut String, modes: &TextModes) {
    if modes.remove_punctuations {
        for token in &modes.remove_tokens {
            *text = text.replace(token, "");
        }
    }
    if modes.only_period {
        for token in &modes.only_period_tokens {
            *text = text.replace(token, "。");
        }
        let mut compacted = String::with_capacity(text.len());
        let mut last_char = '\0';
        for ch in text.chars() {
            if ch == '。' && last_char == '。' {
                continue;
            }
            if compacted.is_empty() && ch == '。' {
                continue;
            }
            last_char = ch;
            compacted.push(ch);
        }
        *text = compacted;
    }
}

fn remove_chars(text: &mut String, text_nop: &[char], comment_strip: &[char]) {
    for ch in text_nop {
        *text = text.replace(*ch, "");
    }
    for ch in comment_strip {
        *text = text.replace(*ch, "");
    }
}

fn count_annotation_slots(working: &str) -> usize {
    let mut total = 0usize;
    let mut temp = working.to_string();
    loop {
        let start = match temp.find('【') {
            Some(idx) => idx,
            None => break,
        };
        let content_start = start + '【'.len_utf8();
        let rel_end = match temp[content_start..].find('】') {
            Some(idx) => idx,
            None => break,
        };
        let content_end = content_start + rel_end;
        let len = temp[content_start..content_end].chars().count();
        if len % 2 == 0 {
            total += len / 2;
        } else {
            total += len / 2 + 1;
        }
        let remove_end = content_end + '】'.len_utf8();
        temp.replace_range(start..remove_end, "");
    }
    total
}

fn strip_annotations(text: &mut String) {
    loop {
        let start = match text.find('【') {
            Some(idx) => idx,
            None => break,
        };
        let content_start = start + '【'.len_utf8();
        let rel_end = match text[content_start..].find('】') {
            Some(idx) => idx,
            None => break,
        };
        let remove_end = content_start + rel_end + '】'.len_utf8();
        text.replace_range(start..remove_end, "");
    }
}

fn missing_spaces(total: usize, row_num: usize) -> usize {
    if row_num == 0 {
        return 0;
    }
    let remainder = total % row_num;
    if remainder == 0 {
        0
    } else {
        row_num - remainder
    }
}

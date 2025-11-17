use anyhow::{Context, Result};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

#[derive(Debug)]
pub struct NumeralMap {
    map: HashMap<usize, String>,
}

impl NumeralMap {
    pub fn load(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        let content =
            fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
        let mut map = HashMap::new();
        for line in content.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }
            let mut parts = trimmed.split('|');
            if let (Some(key), Some(value)) = (parts.next(), parts.next()) {
                if let Ok(num) = key.parse::<usize>() {
                    map.insert(num, value.to_string());
                }
            }
        }
        Ok(Self { map })
    }

    pub fn get(&self, num: usize) -> Option<&str> {
        self.map.get(&num).map(|s| s.as_str())
    }

    pub fn render(&self, num: usize) -> String {
        if let Some(value) = self.get(num) {
            value.to_string()
        } else {
            fallback_digits(num)
        }
    }
}

fn fallback_digits(mut num: usize) -> String {
    if num == 0 {
        return "〇".to_string();
    }
    let digits = ['〇', '一', '二', '三', '四', '五', '六', '七', '八', '九'];
    let mut buf = Vec::new();
    while num > 0 {
        let d = num % 10;
        buf.push(digits[d]);
        num /= 10;
    }
    buf.into_iter().rev().collect()
}

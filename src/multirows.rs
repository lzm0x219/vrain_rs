#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MultiRowsMode {
    Disabled,
    HorizontalLeaf { rows: usize },
    HorizontalPage { rows: usize },
}

impl MultiRowsMode {
    pub fn from_flags(enabled: bool, count: usize, layout_flag: usize) -> Self {
        if !enabled || count <= 1 {
            return MultiRowsMode::Disabled;
        }
        match layout_flag {
            1 => MultiRowsMode::HorizontalLeaf { rows: count },
            2 => MultiRowsMode::HorizontalPage { rows: count },
            _ => MultiRowsMode::HorizontalLeaf { rows: count },
        }
    }
}

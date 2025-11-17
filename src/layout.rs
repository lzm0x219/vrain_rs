#![allow(dead_code)]

use crate::config::{BookConfig, CanvasConfig};
use crate::multirows::MultiRowsMode;
use anyhow::{Result, anyhow};

#[derive(Debug, Clone, Copy)]
pub struct Cell {
    pub x: f32,
    pub y: f32,
}

pub type CellPosition = Cell;

#[derive(Debug, Clone)]
pub struct Layout {
    pub per_page: usize,
    pub pos_l: Vec<Cell>,
    pub pos_r: Vec<Cell>,
    pub cw: f32,
    pub rh: f32,
    pub canvas_width: f32,
    pub canvas_height: f32,
    pub margins_top: f32,
    pub margins_bottom: f32,
    pub rows_per_column: usize,
    pub columns: usize,
    pub multirows_bands: usize,
}

impl Layout {
    pub fn build(
        book: &BookConfig,
        canvas: &CanvasConfig,
        multirows: MultiRowsMode,
    ) -> Result<Self> {
        let col_num = canvas.leaf_col;
        let row_num = book.row_num;
        if col_num == 0 || row_num == 0 {
            return Err(anyhow!("canvas columns/row_num must be > 0"));
        }

        let cw = (canvas.canvas_width
            - canvas.margins_left
            - canvas.margins_right
            - canvas.leaf_center_width)
            / col_num as f32;
        let rh =
            (canvas.canvas_height - canvas.margins_top - canvas.margins_bottom) / row_num as f32;

        let rows_per_column = match multirows {
            MultiRowsMode::Disabled => row_num,
            MultiRowsMode::HorizontalLeaf { rows } | MultiRowsMode::HorizontalPage { rows } => {
                if rows == 0 || row_num % rows != 0 {
                    return Err(anyhow!(
                        "row_num {} not divisible by multirows {}",
                        row_num,
                        rows
                    ));
                }
                row_num / rows
            }
        };

        let per_page = col_num * row_num;

        let mut pos_l = Vec::with_capacity(per_page + 1);
        let mut pos_r = Vec::with_capacity(per_page + 1);
        pos_l.push(Cell { x: 0.0, y: 0.0 });
        pos_r.push(Cell { x: 0.0, y: 0.0 });

        let push_position =
            |pos_x: f32, pos_y: f32, pos_l: &mut Vec<Cell>, pos_r: &mut Vec<Cell>| {
                let mut x = (pos_x * 1000.0).round() / 1000.0;
                let mut y = (pos_y * 1000.0).round() / 1000.0;
                if x.is_nan() {
                    x = 0.0;
                }
                if y.is_nan() {
                    y = 0.0;
                }
                pos_l.push(Cell { x, y });
                pos_r.push(Cell { x: x + cw / 2.0, y });
            };

        match multirows {
            MultiRowsMode::Disabled => {
                for i in 1..=col_num {
                    let base_x = if (i as f32) <= (col_num as f32) / 2.0 {
                        canvas.canvas_width - canvas.margins_right - cw * i as f32
                    } else {
                        canvas.canvas_width
                            - canvas.margins_right
                            - cw * i as f32
                            - canvas.leaf_center_width
                    };
                    for j in 1..=rows_per_column {
                        let pos_y = canvas.canvas_height - canvas.margins_top - rh * j as f32
                            + book.row_delta_y;
                        push_position(base_x, pos_y, &mut pos_l, &mut pos_r);
                    }
                }
            }
            MultiRowsMode::HorizontalLeaf { rows } => {
                for rid in 0..rows {
                    let band_offset = rows_per_column as f32 * rid as f32 * rh;
                    for i in 1..=col_num {
                        let base_x = if (i as f32) <= (col_num as f32) / 2.0 {
                            canvas.canvas_width - canvas.margins_right - cw * i as f32
                        } else {
                            canvas.canvas_width
                                - canvas.margins_right
                                - cw * i as f32
                                - canvas.leaf_center_width
                        };
                        for j in 1..=rows_per_column {
                            let pos_y = canvas.canvas_height
                                - canvas.margins_top
                                - band_offset
                                - rh * j as f32
                                + book.row_delta_y;
                            push_position(base_x, pos_y, &mut pos_l, &mut pos_r);
                        }
                    }
                }
            }
            MultiRowsMode::HorizontalPage { rows } => {
                let half = col_num / 2;
                if half > 0 {
                    for rid in 0..rows {
                        let band_offset = rows_per_column as f32 * rid as f32 * rh;
                        for i in 1..=half {
                            let base_x = canvas.canvas_width - canvas.margins_right - cw * i as f32;
                            for j in 1..=rows_per_column {
                                let pos_y = canvas.canvas_height
                                    - canvas.margins_top
                                    - band_offset
                                    - rh * j as f32
                                    + book.row_delta_y;
                                push_position(base_x, pos_y, &mut pos_l, &mut pos_r);
                            }
                        }
                    }
                }
                for rid in 0..rows {
                    let band_offset = rows_per_column as f32 * rid as f32 * rh;
                    for i in (half + 1)..=col_num {
                        let base_x = canvas.canvas_width
                            - canvas.margins_right
                            - cw * i as f32
                            - canvas.leaf_center_width;
                        for j in 1..=rows_per_column {
                            let pos_y = canvas.canvas_height
                                - canvas.margins_top
                                - band_offset
                                - rh * j as f32
                                + book.row_delta_y;
                            push_position(base_x, pos_y, &mut pos_l, &mut pos_r);
                        }
                    }
                }
            }
        }

        Ok(Self {
            per_page,
            pos_l,
            pos_r,
            cw,
            rh,
            canvas_width: canvas.canvas_width,
            canvas_height: canvas.canvas_height,
            margins_top: canvas.margins_top,
            margins_bottom: canvas.margins_bottom,
            rows_per_column,
            columns: col_num,
            multirows_bands: match multirows {
                MultiRowsMode::Disabled => 1,
                MultiRowsMode::HorizontalLeaf { rows } | MultiRowsMode::HorizontalPage { rows } => {
                    rows
                }
            },
        })
    }

    pub fn capacity(&self) -> usize {
        self.per_page
    }

    pub fn pos_left(&self, idx: usize) -> Option<&Cell> {
        self.pos_l.get(idx)
    }

    pub fn pos_right(&self, idx: usize) -> Option<&Cell> {
        self.pos_r.get(idx)
    }
}

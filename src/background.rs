use crate::config::CanvasConfig;
use image::{ImageBuffer, Rgba, RgbaImage};

// 简单的宣纸/竹简纹理生成器，用于在缺少背景图时兜底
pub fn generate_bamboo_background(canvas: &CanvasConfig) -> image::DynamicImage {
    let width = canvas.canvas_width.max(1.0) as u32;
    let height = canvas.canvas_height.max(1.0) as u32;

    let bc0 = [210u8, 200, 190, 255]; // 底色
    let bc1 = [233u8, 189, 96, 255]; // 竹简色块
    let bc2 = [148u8, 112, 55, 255]; // 韦编/绑带

    let mut img: RgbaImage = ImageBuffer::from_fn(width, height, |_x, _y| Rgba(bc0));

    // 每列区域
    let cw = (canvas.canvas_width
        - canvas.margins_left
        - canvas.margins_right
        - canvas.leaf_center_width)
        / canvas.leaf_col.max(1) as f32;

    for col in 0..canvas.leaf_col {
        let x_start = canvas.margins_left + cw * col as f32 + cw * 0.05;
        let x_end = canvas.margins_left + cw * (col + 1) as f32 - cw * 0.05;
        let y_start = canvas.margins_top;
        let y_end = canvas.canvas_height - canvas.margins_bottom;

        fill_rect(&mut img, x_start, y_start, x_end, y_end, bc1);

        // 右侧淡阴影
        draw_line(&mut img, x_end, y_start, x_end, y_end, [210, 210, 210, 255], 2.0);
        draw_line(&mut img, x_start, y_end, x_end, y_end, [210, 210, 210, 255], 2.0);

        // 顶/底部绑带
        let band_h = (cw * 0.1).max(4.0);
        fill_rect(
            &mut img,
            x_start - cw * 0.02,
            y_start - band_h,
            x_end + cw * 0.02,
            y_start,
            bc2,
        );
        fill_rect(
            &mut img,
            x_start - cw * 0.02,
            y_end,
            x_end + cw * 0.02,
            y_end + band_h,
            bc2,
        );

        // 绑线
        let step = cw / 10.0;
        for j in 0..10 {
            if j == 5 {
                continue;
            }
            let lx = x_start - cw * 0.02 + step * j as f32;
            draw_line(
                &mut img,
                lx,
                y_start - band_h,
                lx + step,
                y_start,
                bc2,
                2.0,
            );
            draw_line(
                &mut img,
                lx,
                y_end,
                lx + step,
                y_end + band_h,
                bc2,
                2.0,
            );
        }

        // 竖向纹理
        let texture_lines = 30;
        for k in 0..texture_lines {
            let t = pseudo_noise(col as u32, k) as f32 / 255.0;
            let gray = 210.0 + 40.0 * t;
            let rx = x_start + cw * 0.1 + cw * 0.8 * (k as f32 / texture_lines as f32);
            let ry1 = y_start + 20.0 * t;
            let ry2 = y_end - 20.0 * t;
            draw_line(
                &mut img,
                rx,
                ry1,
                rx,
                ry2,
                [gray as u8, gray as u8, gray as u8, 255],
                1.0,
            );
        }
    }

    // 全局微弱噪声
    add_noise(&mut img, 0.03);

    image::DynamicImage::ImageRgba8(img)
}

fn fill_rect(img: &mut RgbaImage, x1: f32, y1: f32, x2: f32, y2: f32, color: [u8; 4]) {
    let (min_x, max_x) = if x1 < x2 { (x1, x2) } else { (x2, x1) };
    let (min_y, max_y) = if y1 < y2 { (y1, y2) } else { (y2, y1) };
    let (w, h) = img.dimensions();
    let mut y = min_y.max(0.0) as u32;
    while (y as f32) < max_y && y < h {
        let mut x = min_x.max(0.0) as u32;
        while (x as f32) < max_x && x < w {
            img.put_pixel(x, y, Rgba(color));
            x += 1;
        }
        y += 1;
    }
}

fn draw_line(img: &mut RgbaImage, x1: f32, y1: f32, x2: f32, y2: f32, color: [u8; 4], width: f32) {
    let dx = x2 - x1;
    let dy = y2 - y1;
    let steps = dx.abs().max(dy.abs()).max(1.0) as usize;
    for i in 0..=steps {
        let t = i as f32 / steps as f32;
        let x = x1 + dx * t;
        let y = y1 + dy * t;
        draw_disc(img, x, y, width / 2.0, color);
    }
}

fn draw_disc(img: &mut RgbaImage, cx: f32, cy: f32, r: f32, color: [u8; 4]) {
    let (w, h) = img.dimensions();
    let r2 = r * r;
    let min_x = (cx - r).floor().max(0.0) as u32;
    let max_x = (cx + r).ceil().min(w as f32 - 1.0) as u32;
    let min_y = (cy - r).floor().max(0.0) as u32;
    let max_y = (cy + r).ceil().min(h as f32 - 1.0) as u32;
    for y in min_y..=max_y {
        for x in min_x..=max_x {
            let dx = x as f32 - cx;
            let dy = y as f32 - cy;
            if dx * dx + dy * dy <= r2 {
                img.put_pixel(x, y, Rgba(color));
            }
        }
    }
}

fn add_noise(img: &mut RgbaImage, magnitude: f32) {
    let (w, h) = img.dimensions();
    for y in 0..h {
        for x in 0..w {
            let noise = pseudo_noise(x, y) as f32 / 255.0;
            let delta = (noise - 0.5) * magnitude * 255.0;
            let mut p = img.get_pixel(x, y).0;
            for c in 0..3 {
                let v = (p[c] as f32 + delta).clamp(0.0, 255.0);
                p[c] = v as u8;
            }
            img.put_pixel(x, y, Rgba(p));
        }
    }
}

fn pseudo_noise(x: u32, y: u32) -> u8 {
    // 简单可重复的哈希噪声，无需额外依赖
    let mut v = x.wrapping_mul(73856093) ^ y.wrapping_mul(19349663);
    v ^= v >> 13;
    v = v.wrapping_mul(0x85ebca6b);
    ((v >> 8) & 0xFF) as u8
}


//! 基于 embedded-graphics 实现的离屏画布模块
//! 用于替换 offscreen-canvas 库

use ab_glyph::{point, FontRef, GlyphId, OutlinedGlyph, PxScale, ScaleFont};
use embedded_graphics::pixelcolor::Rgb888;
use embedded_graphics::prelude::*;
use embedded_graphics::primitives::{PrimitiveStyle, PrimitiveStyleBuilder};
use image::imageops::{overlay, FilterType};
use image::{Pixel, Rgb, Rgba, RgbaImage};

// 颜色常量
pub const BLACK: Rgba<u8> = Rgba([0, 0, 0, 255]);
pub const WHITE: Rgba<u8> = Rgba([255, 255, 255, 255]);
pub const BLUE: Rgba<u8> = Rgba([0, 0, 255, 255]);

/// 矩形结构体
#[derive(Debug, Clone, Copy, Default)]
pub struct Rect {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

impl Rect {
    pub fn new(left: i32, top: i32, right: i32, bottom: i32) -> Self {
        Self { left, top, right, bottom }
    }

    pub fn from(x: i32, y: i32, width: i32, height: i32) -> Self {
        Self {
            left: x,
            top: y,
            right: x + width,
            bottom: y + height,
        }
    }

    pub fn width(&self) -> i32 {
        self.right - self.left
    }

    pub fn height(&self) -> i32 {
        self.bottom - self.top
    }

    pub fn set_size(&mut self, width: i32, height: i32) {
        let cx = (self.left + self.right) / 2;
        let cy = (self.top + self.bottom) / 2;
        self.left = cx - width / 2;
        self.right = cx + width / 2;
        self.top = cy - height / 2;
        self.bottom = cy + height / 2;
    }
}

/// 缩放选项
#[derive(Debug, Clone, Copy)]
pub struct ResizeOption {
    pub nwidth: u32,
    pub nheight: u32,
    pub filter: FilterType,
}

/// 旋转选项
#[derive(Debug, Clone, Copy)]
pub struct RotateOption {
    pub center: (f32, f32),
    pub angle: f32,
}

impl RotateOption {
    pub fn from(center: (f32, f32), angle: f32) -> Self {
        Self { center, angle }
    }
}

/// 字体设置
#[derive(Debug, Clone, Copy, Default)]
pub struct FontSettings;

/// 字体类型 (包装 ab_glyph::FontRef)
#[derive(Clone)]
pub struct Font {
    data: Vec<u8>,
}

impl Font {
    pub fn from_bytes(data: &[u8], _settings: FontSettings) -> Result<Self, String> {
        // 验证字体数据是否有效
        FontRef::try_from_slice(data).map_err(|e| format!("{:?}", e))?;
        Ok(Self { data: data.to_vec() })
    }

    fn as_font_ref(&self) -> FontRef<'_> {
        FontRef::try_from_slice(&self.data).unwrap()
    }
}

/// 离屏画布
pub struct OffscreenCanvas {
    buffer: RgbaImage,
    font: Font,
}

impl OffscreenCanvas {
    pub fn new(width: u32, height: u32, font: Font) -> Self {
        Self {
            buffer: RgbaImage::new(width, height),
            font,
        }
    }

    pub fn width(&self) -> u32 {
        self.buffer.width()
    }

    pub fn height(&self) -> u32 {
        self.buffer.height()
    }

    pub fn font(&self) -> &Font {
        &self.font
    }

    /// 清屏
    pub fn clear(&mut self, color: Rgba<u8>) {
        for pixel in self.buffer.pixels_mut() {
            *pixel = color;
        }
    }

    /// 获取图像数据
    pub fn image_data(&self) -> &RgbaImage {
        &self.buffer
    }

    /// 设置单个像素
    #[inline]
    fn set_pixel(&mut self, x: i32, y: i32, color: Rgba<u8>) {
        if x >= 0 && y >= 0 && (x as u32) < self.buffer.width() && (y as u32) < self.buffer.height() {
            self.buffer.put_pixel(x as u32, y as u32, color);
        }
    }

    /// 填充矩形
    pub fn fill_rect(&mut self, rect: Rect, color: Rgba<u8>) {
        let rgb = Rgb888::new(color.0[0], color.0[1], color.0[2]);
        let style = PrimitiveStyle::with_fill(rgb);
        let eg_rect = embedded_graphics::primitives::Rectangle::new(
            Point::new(rect.left, rect.top),
            Size::new(rect.width() as u32, rect.height() as u32),
        );
        for p in eg_rect.into_styled(style).pixels() {
            self.set_pixel(p.0.x, p.0.y, color);
        }
    }

    /// 描边矩形
    pub fn stroke_rect(&mut self, rect: Rect, color: Rgba<u8>) {
        let rgb = Rgb888::new(color.0[0], color.0[1], color.0[2]);
        let style = PrimitiveStyle::with_stroke(rgb, 1);
        let eg_rect = embedded_graphics::primitives::Rectangle::new(
            Point::new(rect.left, rect.top),
            Size::new(rect.width() as u32, rect.height() as u32),
        );
        for p in eg_rect.into_styled(style).pixels() {
            self.set_pixel(p.0.x, p.0.y, color);
        }
    }

    /// 测量文本尺寸
    pub fn measure_text(&self, text: &str, font_size: f32) -> Rect {
        let font_ref = self.font.as_font_ref();
        let (w, h) = layout_glyphs(font_size, &font_ref, text, |_, _| {});
        Rect::from(0, 0, w as i32, h as i32)
    }

    /// 绘制文本
    pub fn draw_text(&mut self, text: &str, color: Rgba<u8>, font_size: f32, x: i32, y: i32) {
        let font_ref = self.font.as_font_ref();
        let w = self.buffer.width() as i32;
        let h = self.buffer.height() as i32;
        draw_text_impl(&mut self.buffer, x, y, &font_ref, font_size, text, color, w, h);
    }

    /// 绘制图像 (支持缩放和旋转)
    pub fn draw_image_at(
        &mut self,
        img: &RgbaImage,
        x: i32,
        y: i32,
        resize_opt: Option<ResizeOption>,
        rotate_opt: Option<RotateOption>,
    ) {
        let mut img = img.clone();
        // 缩放
        if let Some(opt) = resize_opt {
            img = image::imageops::resize(&img, opt.nwidth, opt.nheight, opt.filter);
        }
        // 旋转
        if let Some(opt) = rotate_opt {
            img = rotate_image(&img, opt.center, opt.angle);
        }
        overlay(&mut self.buffer, &img, x as i64, y as i64);
    }

    /// 绘制图像 (源矩形到目标矩形)
    pub fn draw_image_with_src_and_dst(
        &mut self,
        img: &RgbaImage,
        _src: &Rect,
        dst: &Rect,
        filter: FilterType,
    ) {
        let resized = image::imageops::resize(
            img,
            dst.width() as u32,
            dst.height() as u32,
            filter,
        );
        overlay(&mut self.buffer, &resized, dst.left as i64, dst.top as i64);
    }

    /// 绘制图像 (带旋转)
    pub fn draw_image_with_src_and_dst_and_rotation(
        &mut self,
        img: &RgbaImage,
        _src: &Rect,
        dst: &Rect,
        rotate_opt: RotateOption,
    ) {
        let resized = image::imageops::resize(
            img,
            dst.width() as u32,
            dst.height() as u32,
            FilterType::Nearest,
        );
        let rotated = rotate_image(&resized, rotate_opt.center, rotate_opt.angle);
        overlay(&mut self.buffer, &rotated, dst.left as i64, dst.top as i64);
    }
}

// ============ 辅助函数 ============

/// 布局字形并计算文本尺寸
fn layout_glyphs<F>(
    scale: impl Into<PxScale> + Copy,
    font: &impl ab_glyph::Font,
    text: &str,
    mut f: F,
) -> (u32, u32)
where
    F: FnMut(OutlinedGlyph, ab_glyph::Rect),
{
    use ab_glyph::Font as AbFont;
    if text.is_empty() {
        return (0, 0);
    }
    let font = font.as_scaled(scale);
    let mut w = 0.0;
    let mut prev: Option<GlyphId> = None;

    for c in text.chars() {
        let glyph_id = font.glyph_id(c);
        let glyph = glyph_id.with_scale_and_position(scale, point(w, font.ascent()));
        w += font.h_advance(glyph_id);
        if let Some(g) = font.outline_glyph(glyph) {
            if let Some(prev) = prev {
                w += font.kern(glyph_id, prev);
            }
            prev = Some(glyph_id);
            let bb = g.px_bounds();
            f(g, bb);
        }
    }

    let w = w.ceil();
    let h = font.height().ceil();
    (w.max(0.0) as u32 + 1, h.max(0.0) as u32)
}

/// 绘制文本到图像
fn draw_text_impl(
    target: &mut RgbaImage,
    x: i32,
    y: i32,
    font: &FontRef<'_>,
    font_size: f32,
    text: &str,
    color: Rgba<u8>,
    image_width: i32,
    image_height: i32,
) {
    layout_glyphs(font_size, font, text, |g, bb| {
        let x_shift = x + bb.min.x.round() as i32;
        let y_shift = y + bb.min.y.round() as i32;
        g.draw(|gx, gy, gv| {
            let image_x = gx as i32 + x_shift;
            let image_y = gy as i32 + y_shift;
            if (0..image_width).contains(&image_x) && (0..image_height).contains(&image_y) {
                let src_pixel = target.get_pixel_mut(image_x as u32, image_y as u32);
                let gv = gv.clamp(0.0, 1.0);
                let blended = weighted_sum(*src_pixel, color, 1.0 - gv, gv);
                *src_pixel = blended;
            }
        })
    });
}

/// 颜色加权混合
#[inline]
fn weighted_sum(p1: Rgba<u8>, p2: Rgba<u8>, w1: f32, w2: f32) -> Rgba<u8> {
    let r = (p1.0[0] as f32 * w1 + p2.0[0] as f32 * w2).round() as u8;
    let g = (p1.0[1] as f32 * w1 + p2.0[1] as f32 * w2).round() as u8;
    let b = (p1.0[2] as f32 * w1 + p2.0[2] as f32 * w2).round() as u8;
    let a = (p1.0[3] as f32 * w1 + p2.0[3] as f32 * w2).round() as u8;
    Rgba([r, g, b, a])
}

/// 旋转图像 (绕中心点旋转指定弧度)
fn rotate_image(img: &RgbaImage, center: (f32, f32), angle: f32) -> RgbaImage {
    let (w, h) = (img.width(), img.height());
    let mut result = RgbaImage::new(w, h);
    let cos_a = angle.cos();
    let sin_a = angle.sin();
    let (cx, cy) = center;

    for y in 0..h {
        for x in 0..w {
            // 计算旋转后的源坐标
            let dx = x as f32 - cx;
            let dy = y as f32 - cy;
            let src_x = (dx * cos_a + dy * sin_a + cx).round() as i32;
            let src_y = (-dx * sin_a + dy * cos_a + cy).round() as i32;
            if src_x >= 0 && src_x < w as i32 && src_y >= 0 && src_y < h as i32 {
                let pixel = img.get_pixel(src_x as u32, src_y as u32);
                result.put_pixel(x, y, *pixel);
            }
        }
    }
    result
}

//! 基于 tiny-skia 实现的离屏画布模块
//! 支持抗锯齿渲染

use ab_glyph::{point, FontRef, GlyphId, OutlinedGlyph, PxScale, ScaleFont};
use image::imageops::FilterType;
use image::{Rgba, RgbaImage};
use tiny_skia::{
    Color, FillRule, Paint, PathBuilder, Pixmap, PixmapPaint, Stroke, Transform,
};

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

/// 离屏画布 (基于 tiny-skia)
pub struct OffscreenCanvas {
    pixmap: Pixmap,
    font: Font,
}

impl OffscreenCanvas {
    pub fn new(width: u32, height: u32, font: Font) -> Self {
        Self {
            pixmap: Pixmap::new(width, height).unwrap_or_else(|| Pixmap::new(1, 1).unwrap()),
            font,
        }
    }

    pub fn width(&self) -> u32 {
        self.pixmap.width()
    }

    pub fn height(&self) -> u32 {
        self.pixmap.height()
    }

    pub fn font(&self) -> &Font {
        &self.font
    }

    /// 清屏
    pub fn clear(&mut self, color: Rgba<u8>) {
        self.pixmap.fill(rgba_to_color(color));
    }

    /// 获取图像数据 (转换为 RgbaImage)
    pub fn image_data(&self) -> RgbaImage {
        let data = self.pixmap.data();
        let width = self.pixmap.width();
        let height = self.pixmap.height();
        // tiny-skia 使用预乘 alpha RGBA 格式，需要转换
        let mut rgba_data = Vec::with_capacity(data.len());
        for chunk in data.chunks(4) {
            let a = chunk[3] as f32 / 255.0;
            if a > 0.0 {
                // 反预乘
                let r = ((chunk[0] as f32 / a).min(255.0)) as u8;
                let g = ((chunk[1] as f32 / a).min(255.0)) as u8;
                let b = ((chunk[2] as f32 / a).min(255.0)) as u8;
                rgba_data.extend_from_slice(&[r, g, b, chunk[3]]);
            } else {
                rgba_data.extend_from_slice(&[0, 0, 0, 0]);
            }
        }
        RgbaImage::from_raw(width, height, rgba_data).unwrap()
    }

    /// 填充矩形 (支持抗锯齿)
    pub fn fill_rect(&mut self, rect: Rect, color: Rgba<u8>) {
        let mut paint = Paint::default();
        paint.set_color(rgba_to_color(color));
        paint.anti_alias = true;

        if let Some(skia_rect) = tiny_skia::Rect::from_xywh(
            rect.left as f32,
            rect.top as f32,
            rect.width() as f32,
            rect.height() as f32,
        ) {
            let path = PathBuilder::from_rect(skia_rect);
            self.pixmap.fill_path(
                &path,
                &paint,
                FillRule::Winding,
                Transform::identity(),
                None,
            );
        }
    }

    /// 描边矩形 (支持抗锯齿)
    pub fn stroke_rect(&mut self, rect: Rect, color: Rgba<u8>) {
        let mut paint = Paint::default();
        paint.set_color(rgba_to_color(color));
        paint.anti_alias = true;

        let stroke = Stroke {
            width: 1.0,
            ..Default::default()
        };

        if let Some(skia_rect) = tiny_skia::Rect::from_xywh(
            rect.left as f32 + 0.5,
            rect.top as f32 + 0.5,
            rect.width() as f32 - 1.0,
            rect.height() as f32 - 1.0,
        ) {
            let path = PathBuilder::from_rect(skia_rect);
            self.pixmap.stroke_path(
                &path,
                &paint,
                &stroke,
                Transform::identity(),
                None,
            );
        }
    }

    /// 测量文本尺寸
    pub fn measure_text(&self, text: &str, font_size: f32) -> Rect {
        let font_ref = self.font.as_font_ref();
        let (w, h) = layout_glyphs(font_size, &font_ref, text, |_, _| {});
        Rect::from(0, 0, w as i32, h as i32)
    }

    /// 绘制文本 (抗锯齿)
    pub fn draw_text(&mut self, text: &str, color: Rgba<u8>, font_size: f32, x: i32, y: i32) {
        let font_ref = self.font.as_font_ref();
        let w = self.pixmap.width() as i32;
        let h = self.pixmap.height() as i32;
        draw_text_impl(&mut self.pixmap, x, y, &font_ref, font_size, text, color, w, h);
    }

    /// 绘制图像 (支持缩放和旋转，带抗锯齿)
    pub fn draw_image_at(
        &mut self,
        img: &RgbaImage,
        x: i32,
        y: i32,
        resize_opt: Option<ResizeOption>,
        rotate_opt: Option<RotateOption>,
    ) {
        let mut img = img.clone();
        // 缩放 (使用高质量插值)
        if let Some(opt) = resize_opt {
            img = image::imageops::resize(&img, opt.nwidth, opt.nheight, opt.filter);
        }
        // 旋转 (使用双线性插值抗锯齿)
        if let Some(opt) = rotate_opt {
            img = rotate_image_bilinear(&img, opt.center, opt.angle);
        }
        self.draw_rgba_image(&img, x, y);
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
        self.draw_rgba_image(&resized, dst.left, dst.top);
    }

    /// 绘制图像 (带旋转，使用双线性插值抗锯齿)
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
            FilterType::Triangle,  // 使用双线性插值
        );
        let rotated = rotate_image_bilinear(&resized, rotate_opt.center, rotate_opt.angle);
        self.draw_rgba_image(&rotated, dst.left, dst.top);
    }

    /// 内部方法: 绘制 RgbaImage 到画布
    fn draw_rgba_image(&mut self, img: &RgbaImage, x: i32, y: i32) {
        let src_pixmap = rgba_image_to_pixmap(img);
        if let Some(ref pm) = src_pixmap {
            self.pixmap.draw_pixmap(
                x,
                y,
                pm.as_ref(),
                &PixmapPaint::default(),
                Transform::identity(),
                None,
            );
        }
    }

    /// 绘制环形进度条 (抗锯齿)
    /// cx, cy: 圆心坐标
    /// radius: 半径
    /// stroke_width: 线宽
    /// start_angle: 起始角度 (度数, 0=上, 90=右, 180=下, 270=左)
    /// percent: 进度百分比 (0.0 - 100.0)
    /// fg_color: 前景色
    /// bg_color: 背景色
    /// round_cap: 是否圆角端点
    pub fn draw_ring_progress(
        &mut self,
        cx: i32,
        cy: i32,
        radius: u32,
        stroke_width: u32,
        start_angle: i32,
        percent: f32,
        fg_color: Rgba<u8>,
        bg_color: Rgba<u8>,
        round_cap: bool,
    ) {
        let cx = cx as f32;
        let cy = cy as f32;
        let radius = radius as f32;
        
        // 绘制背景圆环 (完整360度)
        self.draw_arc(cx, cy, radius, 0.0, 360.0, stroke_width as f32, bg_color, round_cap);

        // 绘制前景进度
        if percent > 0.0 {
            // 用户坐标系: 0度=上, 顺时针
            // 转换到标准坐标系: 0度=右, 逆时针 -> start_angle - 90
            let start = (start_angle - 90) as f32;
            let sweep = 360.0 * (percent / 100.0);
            self.draw_arc(cx, cy, radius, start, sweep, stroke_width as f32, fg_color, round_cap);
        }
    }

    /// 绘制圆弧 (抗锯齿)
    fn draw_arc(
        &mut self,
        cx: f32,
        cy: f32,
        radius: f32,
        start_deg: f32,
        sweep_deg: f32,
        stroke_width: f32,
        color: Rgba<u8>,
        round_cap: bool,
    ) {
        if sweep_deg.abs() < 0.01 {
            return;
        }

        let mut paint = Paint::default();
        paint.set_color(rgba_to_color(color));
        paint.anti_alias = true;

        let mut stroke = Stroke {
            width: stroke_width,
            ..Default::default()
        };
        if round_cap {
            stroke.line_cap = tiny_skia::LineCap::Round;
        }

        // 构建圆弧路径
        let path = build_arc_path(cx, cy, radius, start_deg, sweep_deg);
        if let Some(p) = path {
            self.pixmap.stroke_path(
                &p,
                &paint,
                &stroke,
                Transform::identity(),
                None,
            );
        }
    }

    /// 绘制填充圆形 (抗锯齿)
    #[allow(dead_code)]
    pub fn fill_circle(&mut self, cx: i32, cy: i32, radius: u32, color: Rgba<u8>) {
        let mut paint = Paint::default();
        paint.set_color(rgba_to_color(color));
        paint.anti_alias = true;

        if let Some(path) = PathBuilder::from_circle(cx as f32, cy as f32, radius as f32) {
            self.pixmap.fill_path(
                &path,
                &paint,
                FillRule::Winding,
                Transform::identity(),
                None,
            );
        }
    }

    /// 绘制描边圆形 (抗锯齿)
    #[allow(dead_code)]
    pub fn stroke_circle(&mut self, cx: i32, cy: i32, radius: u32, stroke_width: f32, color: Rgba<u8>) {
        let mut paint = Paint::default();
        paint.set_color(rgba_to_color(color));
        paint.anti_alias = true;

        let stroke = Stroke {
            width: stroke_width,
            ..Default::default()
        };

        if let Some(path) = PathBuilder::from_circle(cx as f32, cy as f32, radius as f32) {
            self.pixmap.stroke_path(
                &path,
                &paint,
                &stroke,
                Transform::identity(),
                None,
            );
        }
    }
}

// ============ 辅助函数 ============

/// Rgba 转换为 tiny-skia Color
#[inline]
fn rgba_to_color(rgba: Rgba<u8>) -> Color {
    Color::from_rgba8(rgba.0[0], rgba.0[1], rgba.0[2], rgba.0[3])
}

/// RgbaImage 转换为 Pixmap
fn rgba_image_to_pixmap(img: &RgbaImage) -> Option<Pixmap> {
    let width = img.width();
    let height = img.height();
    let mut pixmap = Pixmap::new(width, height)?;
    
    // 转换为预乘 alpha 格式
    let src = img.as_raw();
    let dst = pixmap.data_mut();
    for i in 0..(width * height) as usize {
        let idx = i * 4;
        let r = src[idx] as f32;
        let g = src[idx + 1] as f32;
        let b = src[idx + 2] as f32;
        let a = src[idx + 3] as f32 / 255.0;
        // 预乘 alpha
        dst[idx] = (r * a) as u8;
        dst[idx + 1] = (g * a) as u8;
        dst[idx + 2] = (b * a) as u8;
        dst[idx + 3] = src[idx + 3];
    }
    Some(pixmap)
}

/// 构建圆弧路径
fn build_arc_path(cx: f32, cy: f32, radius: f32, start_deg: f32, sweep_deg: f32) -> Option<tiny_skia::Path> {
    let mut pb = PathBuilder::new();
    
    // 将度数转换为弧度
    let start_rad = start_deg.to_radians();
    let sweep_rad = sweep_deg.to_radians();
    
    // 起点
    let start_x = cx + radius * start_rad.cos();
    let start_y = cy + radius * start_rad.sin();
    pb.move_to(start_x, start_y);
    
    // 使用多个小段近似圆弧 (每段最大 90 度)
    let segments = ((sweep_deg.abs() / 90.0).ceil() as i32).max(1);
    let segment_sweep = sweep_rad / segments as f32;
    
    for i in 0..segments {
        let seg_start = start_rad + segment_sweep * i as f32;
        let _seg_end = seg_start + segment_sweep;
        
        // 使用贝塞尔曲线近似圆弧
        let (cp1x, cp1y, cp2x, cp2y, ex, ey) = 
            arc_to_bezier(cx, cy, radius, seg_start, segment_sweep);
        pb.cubic_to(cp1x, cp1y, cp2x, cp2y, ex, ey);
    }
    
    pb.finish()
}

/// 将圆弧转换为三次贝塞尔曲线的控制点
fn arc_to_bezier(cx: f32, cy: f32, r: f32, start: f32, sweep: f32) -> (f32, f32, f32, f32, f32, f32) {
    let end = start + sweep;
    
    // 控制点距离系数
    let alpha = (4.0 / 3.0) * (sweep / 4.0).tan();
    
    let cos_start = start.cos();
    let sin_start = start.sin();
    let cos_end = end.cos();
    let sin_end = end.sin();
    
    let p1x = cx + r * cos_start;
    let p1y = cy + r * sin_start;
    let p2x = cx + r * cos_end;
    let p2y = cy + r * sin_end;
    
    let cp1x = p1x - alpha * r * sin_start;
    let cp1y = p1y + alpha * r * cos_start;
    let cp2x = p2x + alpha * r * sin_end;
    let cp2y = p2y - alpha * r * cos_end;
    
    (cp1x, cp1y, cp2x, cp2y, p2x, p2y)
}

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
    #[allow(unused_imports)]
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

/// 绘制文本到 Pixmap (抗锯齿)
fn draw_text_impl(
    pixmap: &mut Pixmap,
    x: i32,
    y: i32,
    font: &FontRef<'_>,
    font_size: f32,
    text: &str,
    color: Rgba<u8>,
    image_width: i32,
    image_height: i32,
) {
    let data = pixmap.data_mut();
    let stride = (image_width * 4) as usize;
    
    layout_glyphs(font_size, font, text, |g, bb| {
        let x_shift = x + bb.min.x.round() as i32;
        let y_shift = y + bb.min.y.round() as i32;
        g.draw(|gx, gy, gv| {
            let image_x = gx as i32 + x_shift;
            let image_y = gy as i32 + y_shift;
            if (0..image_width).contains(&image_x) && (0..image_height).contains(&image_y) {
                let idx = (image_y as usize) * stride + (image_x as usize) * 4;
                if idx + 3 < data.len() {
                    let gv = gv.clamp(0.0, 1.0);
                    // 读取当前像素 (预乘格式)
                    let dst_r = data[idx] as f32;
                    let dst_g = data[idx + 1] as f32;
                    let dst_b = data[idx + 2] as f32;
                    let dst_a = data[idx + 3] as f32 / 255.0;
                    
                    // 源颜色 (预乘)
                    let src_a = (color.0[3] as f32 / 255.0) * gv;
                    let src_r = color.0[0] as f32 * src_a;
                    let src_g = color.0[1] as f32 * src_a;
                    let src_b = color.0[2] as f32 * src_a;
                    
                    // Porter-Duff over 混合
                    let out_a = src_a + dst_a * (1.0 - src_a);
                    let out_r = src_r + dst_r * (1.0 - src_a);
                    let out_g = src_g + dst_g * (1.0 - src_a);
                    let out_b = src_b + dst_b * (1.0 - src_a);
                    
                    data[idx] = out_r.min(255.0) as u8;
                    data[idx + 1] = out_g.min(255.0) as u8;
                    data[idx + 2] = out_b.min(255.0) as u8;
                    data[idx + 3] = (out_a * 255.0).min(255.0) as u8;
                }
            }
        })
    });
}

/// 双线性插值旋转图像 (抗锯齿)
fn rotate_image_bilinear(img: &RgbaImage, center: (f32, f32), angle: f32) -> RgbaImage {
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
            let src_x = dx * cos_a + dy * sin_a + cx;
            let src_y = -dx * sin_a + dy * cos_a + cy;
            
            // 双线性插值
            if src_x >= 0.0 && src_x < (w - 1) as f32 && src_y >= 0.0 && src_y < (h - 1) as f32 {
                let x0 = src_x.floor() as u32;
                let y0 = src_y.floor() as u32;
                let fx = src_x - x0 as f32;
                let fy = src_y - y0 as f32;
                
                let p00 = img.get_pixel(x0, y0);
                let p10 = img.get_pixel(x0 + 1, y0);
                let p01 = img.get_pixel(x0, y0 + 1);
                let p11 = img.get_pixel(x0 + 1, y0 + 1);
                
                // 混合四个像素
                let pixel = bilinear_blend(p00, p10, p01, p11, fx, fy);
                result.put_pixel(x, y, pixel);
            }
        }
    }
    result
}

/// 双线性混合四个像素
#[inline]
fn bilinear_blend(
    p00: &Rgba<u8>,
    p10: &Rgba<u8>,
    p01: &Rgba<u8>,
    p11: &Rgba<u8>,
    fx: f32,
    fy: f32,
) -> Rgba<u8> {
    let inv_fx = 1.0 - fx;
    let inv_fy = 1.0 - fy;
    
    let w00 = inv_fx * inv_fy;
    let w10 = fx * inv_fy;
    let w01 = inv_fx * fy;
    let w11 = fx * fy;
    
    let r = (p00.0[0] as f32 * w00 + p10.0[0] as f32 * w10 + p01.0[0] as f32 * w01 + p11.0[0] as f32 * w11).round() as u8;
    let g = (p00.0[1] as f32 * w00 + p10.0[1] as f32 * w10 + p01.0[1] as f32 * w01 + p11.0[1] as f32 * w11).round() as u8;
    let b = (p00.0[2] as f32 * w00 + p10.0[2] as f32 * w10 + p01.0[2] as f32 * w01 + p11.0[2] as f32 * w11).round() as u8;
    let a = (p00.0[3] as f32 * w00 + p10.0[3] as f32 * w10 + p01.0[3] as f32 * w01 + p11.0[3] as f32 * w11).round() as u8;
    
    Rgba([r, g, b, a])
}

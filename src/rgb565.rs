/// RGB888转RGB565 (使用四舍五入减少偏色)
/// 原方法直接截断低位会导致颜色整体偏暗，改用四舍五入可减少误差
#[inline]
pub fn rgb_to_rgb565(r: u8, g: u8, b: u8) -> u16 {
    // 四舍五入: 加上被截断位的一半，然后截断
    // R: 截断3位，加4(0b100)后再截断
    // G: 截断2位，加2(0b10)后再截断  
    // B: 截断3位，加4(0b100)后再截断
    let r5 = ((r as u16 + 4).min(255) >> 3) as u16;
    let g6 = ((g as u16 + 2).min(255) >> 2) as u16;
    let b5 = ((b as u16 + 4).min(255) >> 3) as u16;
    
    (r5 << 11) | (g6 << 5) | b5
}

pub fn rgb888_to_rgb565_be(img: &[u8], width: usize, height: usize) -> Vec<u8>{
    let mut rgb565 = Vec::with_capacity(width * height * 2);
    for p in img.chunks(3){
        let rgb565_pixel = rgb_to_rgb565(p[0], p[1], p[2]);
        rgb565.extend_from_slice(&rgb565_pixel.to_be_bytes());
    }
    rgb565
}
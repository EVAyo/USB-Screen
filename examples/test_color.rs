use anyhow::{Ok, Result, anyhow};
include!("../src/rgb565.rs");

//验证图像处理转换后是否会产生变色

fn main() -> Result<()> {
    //加载保存的原始屏幕图像
    let img = image::open("examples/last_image.png")?.to_rgb8();
    let width = img.width() as usize;
    let height = img.height() as usize;
    
    //转换为RGB565格式
    let rgb565 = rgb888_to_rgb565_be(&img, width, height);
    //模拟通过USB串口发送图像数据
    let send_data = draw_rgb565_serial(&rgb565, 0, 0, width as u16, height as u16)?;
    //模拟单片机解压还原图像数据
    let received_rgb565 = receive_rgb565_serial(send_data)?;

    //将received_rgb565转换回RGB888并保存为PNG
    let received_rgb888 = rgb565_be_to_rgb888(&received_rgb565, width, height);
    let received_img = image::RgbImage::from_raw(width as u32, height as u32, received_rgb888.clone())
        .ok_or(anyhow!("Failed to create image from RGB888 data"))?;
    received_img.save("examples/received_image.png")?;
    
    println!("原图: examples/last_image.png");
    println!("转换后: examples/received_image.png");
    
    // 分析颜色差异
    analyze_color_difference(&img, &received_rgb888, width, height);
    
    Ok(())
}

/// 分析原图与转换后图像的颜色差异
fn analyze_color_difference(original: &[u8], converted: &[u8], width: usize, height: usize) {
    let pixel_count = width * height;
    
    // 统计各通道的差异
    let mut r_diff_sum: i64 = 0;
    let mut g_diff_sum: i64 = 0;
    let mut b_diff_sum: i64 = 0;
    let mut max_r_diff: i32 = 0;
    let mut max_g_diff: i32 = 0;
    let mut max_b_diff: i32 = 0;
    
    for i in 0..pixel_count {
        let orig_r = original[i * 3] as i32;
        let orig_g = original[i * 3 + 1] as i32;
        let orig_b = original[i * 3 + 2] as i32;
        
        let conv_r = converted[i * 3] as i32;
        let conv_g = converted[i * 3 + 1] as i32;
        let conv_b = converted[i * 3 + 2] as i32;
        
        // 计算差异 (转换后 - 原始，正值表示转换后更亮)
        let dr = conv_r - orig_r;
        let dg = conv_g - orig_g;
        let db = conv_b - orig_b;
        
        r_diff_sum += dr as i64;
        g_diff_sum += dg as i64;
        b_diff_sum += db as i64;
        
        max_r_diff = max_r_diff.max(dr.abs());
        max_g_diff = max_g_diff.max(dg.abs());
        max_b_diff = max_b_diff.max(db.abs());
    }
    
    let avg_r = r_diff_sum as f64 / pixel_count as f64;
    let avg_g = g_diff_sum as f64 / pixel_count as f64;
    let avg_b = b_diff_sum as f64 / pixel_count as f64;
    
    println!("\n=== 颜色差异分析 (转换后 - 原始) ===");
    println!("像素总数: {}", pixel_count);
    println!("R通道: 平均差异={:+.2}, 最大差异={}", avg_r, max_r_diff);
    println!("G通道: 平均差异={:+.2}, 最大差异={}", avg_g, max_g_diff);
    println!("B通道: 平均差异={:+.2}, 最大差异={}", avg_b, max_b_diff);
    
    // 判断偏色方向
    println!("\n=== 偏色诊断 ===");
    if avg_b > avg_r + 1.0 && avg_b > avg_g + 1.0 {
        println!("诊断: 图像偏蓝 (B通道相对偏高)");
    } else if avg_r > avg_b + 1.0 && avg_r > avg_g + 1.0 {
        println!("诊断: 图像偏红 (R通道相对偏高)");
    } else if avg_g > avg_r + 1.0 && avg_g > avg_b + 1.0 {
        println!("诊断: 图像偏绿 (G通道相对偏高)");
    } else if (avg_r - avg_g).abs() < 1.0 && (avg_g - avg_b).abs() < 1.0 && (avg_r - avg_b).abs() < 1.0 {
        println!("诊断: RGB565量化误差正常，无明显偏色");
        println!("注意: RGB565量化会导致最大约4-8的精度损失，这是正常的");
    } else {
        println!("诊断: 轻微色差，可能是RGB565量化导致");
    }
    
    // 理论上RGB565转换的最大误差
    println!("\n=== 理论误差说明 ===");
    println!("RGB565量化理论最大误差: R/B约7(3位), G约3(2位)");
    println!("如果实际最大差异超过此范围，说明可能存在编码/解码问题");
}

/// 将RGB565大端序字节数组转换回RGB888
/// 输入: RGB565 大端序字节数组 (每像素2字节)
/// 输出: RGB888 字节数组 (每像素3字节)
pub fn rgb565_be_to_rgb888(rgb565: &[u8], width: usize, height: usize) -> Vec<u8> {
    let pixel_count = width * height;
    let mut rgb888 = Vec::with_capacity(pixel_count * 3);
    
    for chunk in rgb565.chunks_exact(2) {
        // 从大端序字节还原u16
        let pixel = u16::from_be_bytes([chunk[0], chunk[1]]);
        
        // 从RGB565提取各通道 (格式: RRRRR GGGGGG BBBBB)
        // R: bits 15-11, G: bits 10-5, B: bits 4-0
        let r5 = ((pixel >> 11) & 0x1F) as u8;  // 5位红色
        let g6 = ((pixel >> 5) & 0x3F) as u8;   // 6位绿色
        let b5 = (pixel & 0x1F) as u8;          // 5位蓝色
        
        // 扩展到8位 (通过左移并填充低位来还原精度)
        // R: 5位 -> 8位: 左移3位，低3位用高3位填充
        // G: 6位 -> 8位: 左移2位，低2位用高2位填充
        // B: 5位 -> 8位: 左移3位，低3位用高3位填充
        let r8 = (r5 << 3) | (r5 >> 2);
        let g8 = (g6 << 2) | (g6 >> 4);
        let b8 = (b5 << 3) | (b5 >> 2);
        
        rgb888.push(r8);
        rgb888.push(g8);
        rgb888.push(b8);
    }
    
    rgb888
}


// 模拟USB串口发送RGB565图像数据，并返回完整帧数据
pub fn draw_rgb565_serial(rgb565:&[u8], x: u16, y: u16, width: u16, height: u16) -> Result<(Vec<u8>, usize, usize)>{
    
    let compressed = lz4_flex::compress_prepend_size(rgb565);
    Ok((compressed, width as usize, height as usize))
}

// 模拟单片机端接收并解压RGB565图像数据
pub fn receive_rgb565_serial((compressed, image_width, image_height):(Vec<u8>, usize, usize)) -> Result<Vec<u8>>{
    let decompressed = lz4_flex::decompress_size_prepended(&compressed)?;
    let screen_w = 240; //假设屏幕宽度
    let screen_h = 240; //假设屏幕高度
    let expected = image_width * image_height * 2;
    println!("LZ4_OK;decompressed={};expected={}\n", decompressed.len(), expected);
    if decompressed.len() != expected {
        Err(anyhow::anyhow!("Decompressed size mismatch"))
    } else {
        println!("DRAW_START;x={};y={};w={};h={};bytes={}\n", 0, 0, image_width, image_height, decompressed.len());
        
        println!("SCREEN_SIZE;w={};h={}\n", screen_w, screen_h);
        
        draw_rgb565_u8array_fast(
            0,
            0,
            image_width as u16,
            image_height as u16,
            &decompressed,
        )
    }
}

/// 模拟ESP32绘制RGB565图像数据到屏幕
pub fn draw_rgb565_u8array_fast(
    x: u16,
    y: u16,
    width: u16,
    height: u16,
    pixels: &[u8],
) -> Result<Vec<u8>> {
    let start_time = std::time::Instant::now();
    let pixel_count = width as usize * height as usize;
    let expected_bytes = pixel_count * 2;
    
    // 验证数据长度
    if pixels.len() != expected_bytes {
        println!("[DRAW_ERR] pixels.len={} expected={} ({}x{})", 
            pixels.len(), expected_bytes, width, height);
        return Err(anyhow!("error: pixels.len() {} != expected {}", pixels.len(), expected_bytes));
    }
    //测试无色调调整绘制
    let adj_r = 0;
    let adj_g = 0;
    let adj_b = 0;
    
    // mipidsi 库的 set_pixels_buffer 始终使用 inclusive 结束坐标
    // 窗口范围为 [x, end_x] x [y, end_y]，宽度为 end_x - x + 1
    let (end_x, end_y) = (x + width - 1, y + height - 1);
    
    // 输出关键绘制信息
    println!("[DRAW] pos=({},{}) size={}x{} window=({},{})..({},{}) bytes={}", 
        x, y, width, height, x, y, end_x, end_y, pixels.len());
    
    // 如果没有色调调整，直接绘制
    let draw_result = if adj_r == 0 && adj_g == 0 && adj_b == 0 {
        //这里回去调用mipidsi的set_pixels_buffer方法 @examples/mipidsi/src/lib.rs:271 
        // match &mut display_manager.display {
        //     DisplayInterface::ST7735s(display) => {
        //         display.set_pixels_buffer(x, y, end_x, end_y, pixels)
        //     }
        //     DisplayInterface::ST7789(display) => {
        //         display.set_pixels_buffer(x, y, end_x, end_y, pixels)
        //     }
        //     DisplayInterface::ST7796(display) => {
        //         display.set_pixels_buffer(x, y, end_x, end_y, pixels)
        //     }
        // }
        Ok(())
    } else {
        //这里回去调用mipidsi的set_pixels_buffer方法 @examples/mipidsi/src/lib.rs:271 
        // 应用色调调整
        println!("[DRAW] color_adjust r={} g={} b={}", adj_r, adj_g, adj_b);
        // let mut adjusted_pixels = Vec::with_capacity(pixels.len());
        // for chunk in pixels.chunks_exact(2) {
        //     let pixel = u16::from_be_bytes([chunk[0], chunk[1]]);
        //     let (r, g, b) = rgb565_to_rgb888_adjusted(pixel, adj_r, adj_g, adj_b);
        //     let adjusted = rgb888_to_rgb565(r, g, b);
        //     adjusted_pixels.extend_from_slice(&adjusted.to_be_bytes());
        // }
        
        // match &mut display_manager.display {
        //     DisplayInterface::ST7735s(display) => {
        //         display.set_pixels_buffer(x, y, end_x, end_y, &adjusted_pixels)
        //     }
        //     DisplayInterface::ST7789(display) => {
        //         display.set_pixels_buffer(x, y, end_x, end_y, &adjusted_pixels)
        //     }
        //     DisplayInterface::ST7796(display) => {
        //         display.set_pixels_buffer(x, y, end_x, end_y, &adjusted_pixels)
        //     }
        // }
        Ok(())
    };
    
    let elapsed_ms = start_time.elapsed().as_millis();
    
    println!("[DRAW_OK] {}x{} pixels in {}ms", width, height, elapsed_ms);
    Ok(pixels.to_vec())
}
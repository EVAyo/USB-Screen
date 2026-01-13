use std::time::{Duration, Instant};
use std::io::{Read, Write};
use std::sync::Mutex;
use std::collections::HashMap;

use futures_lite::future::block_on;
use image::{Rgb, RgbImage};
use log::{info, warn, debug};
use nusb::Interface;
use anyhow::{anyhow, Result};
use once_cell::sync::Lazy;
#[cfg(feature = "usb-serial")]
use serialport::{SerialPort, SerialPortInfo, SerialPortType};

use crate::rgb565::rgb888_to_rgb565_be;

// ESP32 WiFi 屏幕使用的高波特率
const ESP32_BAUD_RATE: u32 = 2_000_000;
// 普通串口屏幕使用的波特率
const DEFAULT_BAUD_RATE: u32 = 115_200;

// ESP32设备缓存: port_name -> (width, height)
// 用于避免重复探测已发现的设备（设备被打开后无法再次探测）
#[cfg(feature = "usb-serial")]
static ESP32_DEVICE_CACHE: Lazy<Mutex<HashMap<String, (u16, u16)>>> = Lazy::new(|| Mutex::new(HashMap::new()));

// use crate::rgb565::rgb888_to_rgb565_be;

const BULK_OUT_EP: u8 = 0x01;
const BULK_IN_EP: u8 = 0x81;

#[derive(Clone, Debug)]
pub struct UsbScreenInfo{
    pub label: String,
    pub address: String,
    pub width: u16,
    pub height: u16,
    // 是否是ESP32 WiFi屏幕(通过ReadInfo探测发现)，需要使用高波特率
    pub is_esp32_wifi: bool,
}

pub enum UsbScreen{
    USBRaw((UsbScreenInfo, Interface)),
    #[cfg(feature = "usb-serial")]
    USBSerial((UsbScreenInfo, Box<dyn SerialPort>))
}

impl UsbScreen{
    pub fn draw_rgb_image(&mut self, x: u16, y: u16, img:&RgbImage) -> anyhow::Result<()>{
        //如果图像比屏幕大， 不绘制，否则会RP2040死机导致卡住
        match self{
            UsbScreen::USBRaw((info, interface)) => {
                if img.width() <= info.width as u32 && img.height() <= info.height as u32{
                    draw_rgb_image(x, y, img, interface)?;
                }
            }

            #[cfg(feature = "usb-serial")]
            UsbScreen::USBSerial((info, port)) => {
                if img.width() <= info.width as u32 && img.height() <= info.height as u32{
                    let rgb565 = rgb888_to_rgb565_be(&img, img.width() as usize, img.height() as usize);
                    if info.is_esp32_wifi {
                        // ESP32设备使用合并发送方式（高速）
                        draw_rgb565_serial(&rgb565, x, y, img.width() as u16, img.height() as u16, port.as_mut())?;
                    } else {
                        // 老设备使用分段发送方式（兼容性更好）
                        draw_rgb565_serial_legacy(&rgb565, x, y, img.width() as u16, img.height() as u16, port.as_mut())?;
                    }
                }
            }
        }
        Ok(())
    }

    pub fn open(info: UsbScreenInfo) -> Result<Self>{
        info!("打开屏幕:label={} addr={} {}x{} esp32={}", info.label, info.address, info.width, info.height, info.is_esp32_wifi);
        let addr = info.address.clone();
        if info.label.contains("Screen"){
            //USB Raw设备, addr是device_address
            Ok(Self::USBRaw((info, open_usb_raw_device(&addr)?)))
        }else{
            #[cfg(feature = "usb-serial")]
            {
                //USB串口设备, addr是串口名称
                // ESP32 WiFi屏幕使用高波特率，普通串口屏幕使用默认波特率
                let baud_rate = if info.is_esp32_wifi { ESP32_BAUD_RATE } else { DEFAULT_BAUD_RATE };
                let screen = serialport::new(&info.address, baud_rate)
                    .timeout(Duration::from_millis(100))
                    .open()?;
                Ok(Self::USBSerial((info, screen)))
            }
            #[cfg(not(feature = "usb-serial"))]
            {
                Err(anyhow!("此平台不支持 USB串口设备"))
            }
        }
    }
}

pub fn find_and_open_a_screen() -> Option<UsbScreen>{
    //先查找串口设备
    let devices = find_all_device();
    for info in devices{
        if let Ok(screen) = UsbScreen::open(info){
            return Some(screen);
        }
    }
    None
}

pub fn open_usb_raw_device(device_address: &str) -> Result<Interface>{
    let di = nusb::list_devices()?;
    for d in di{
        if d.serial_number().unwrap_or("").starts_with("USBSCR") && d.device_address() == device_address.parse::<u8>()?{
            let device = d.open()?;
            let interface = device.claim_interface(0)?;
            return Ok(interface);
        }
    }
    Err(anyhow!("设备地址未找到"))
}

fn get_screen_size_from_serial_number(serial_number:&str) -> (u16, u16){
    //从串号中读取屏幕大小
    let screen_size = &serial_number[6..serial_number.find(";").unwrap_or(13)];
    let screen_size = screen_size.replace("X", "x");
    let mut arr = screen_size.split("x");
    let width = arr.next().unwrap_or("160").parse::<u16>().unwrap_or(160);
    let height = arr.next().unwrap_or("128").parse::<u16>().unwrap_or(128);
    (width, height)
}

// 查询所有USB屏幕设备
// 对于USB Raw返回的第2个参数是 device_address
// 对于USB Serial, 返回的第2个参数是串口名称
pub fn find_all_device() -> Vec<UsbScreenInfo>{
    let mut devices = vec![];
    if let Ok(di) = nusb::list_devices(){
        for d in di{
            #[cfg(not(windows))]
            info!("USB Raw设备:{:?}", d);
            let serial_number = d.serial_number().unwrap_or("");
            if  d.product_string().unwrap_or("") == "USB Screen" && serial_number.starts_with("USBSCR"){
                let label = format!("USB Screen({})", d.device_address());
                let address = format!("{}", d.device_address());
                let (width, height) = get_screen_size_from_serial_number(serial_number);
                devices.push(UsbScreenInfo{
                    label,
                    address,
                    width,
                    height,
                    is_esp32_wifi: false,
                });
            }
        }
    }
    // println!("USB Raw设备数量:{}", devices.len());
    #[cfg(feature = "usb-serial")]
    devices.extend_from_slice(&find_usb_serial_device());
    #[cfg(not(windows))]
    info!("所有usb 设备:{:?}", devices);

    if devices.len() == 0{
        warn!("no available device!");
    }

    devices
}

/// 通过发送ReadInfo命令探测串口是否是ESP32 WiFi屏幕
/// 返回 Some((width, height)) 如果探测成功
#[cfg(feature = "usb-serial")]
fn probe_port_with_readinfo(port_name: &str, timeout_ms: u64) -> Option<(u16, u16)> {
    let timeout = Duration::from_millis(timeout_ms);
    
    // 尝试打开串口
    let mut port = match serialport::new(port_name, DEFAULT_BAUD_RATE)
        .timeout(Duration::from_millis(200))
        .open() {
        Ok(p) => p,
        Err(_) => return None,
    };

    // 清空缓冲区
    let mut drain_buf = [0u8; 1024];
    let _ = port.read(&mut drain_buf);

    // 发送 ReadInfo 命令
    if port.write_all(b"ReadInfo\n").is_err() {
        return None;
    }
    let _ = port.flush();

    // 等待响应
    let start = Instant::now();
    let mut buf: Vec<u8> = Vec::new();
    let mut tmp = [0u8; 256];
    
    while start.elapsed() < timeout {
        match port.read(&mut tmp) {
            Ok(n) if n > 0 => {
                buf.extend_from_slice(&tmp[..n]);
                // 查找换行符
                if let Some(pos) = buf.iter().position(|&b| b == b'\n') {
                    let line = String::from_utf8_lossy(&buf[..pos]).to_string();
                    debug!("ReadInfo响应: {}", line);
                    
                    // 解析响应: ESP32-WIFI-SCREEN;{width};{height};PROTO:USB-SCREEN
                    if let Some(idx) = line.to_uppercase().find("ESP32-WIFI-SCREEN") {
                        let payload = &line[idx..];
                        if payload.contains("PROTO:USB-SCREEN") {
                            let parts: Vec<&str> = payload.split(';').collect();
                            if parts.len() >= 4 {
                                let w = parts.get(1).and_then(|s| s.parse::<u16>().ok());
                                let h = parts.get(2).and_then(|s| s.parse::<u16>().ok());
                                if let (Some(w), Some(h)) = (w, h) {
                                    if w > 0 && h > 0 {
                                        return Some((w, h));
                                    }
                                }
                            }
                            // 格式不对但确实是ESP32屏幕，使用默认尺寸
                            return Some((240, 240));
                        }
                    }
                    break;
                }
            }
            Ok(_) => {}
            Err(ref e) if e.kind() == std::io::ErrorKind::TimedOut => {}
            Err(_) => break,
        }
    }
    None
}

#[cfg(feature = "usb-serial")]
pub fn find_usb_serial_device() -> Vec<UsbScreenInfo>{
    let ports: Vec<SerialPortInfo> = serialport::available_ports().unwrap_or(vec![]);
    let mut devices = vec![];
    // 记录已经通过串号识别的端口，避免重复探测
    let mut identified_ports: Vec<String> = vec![];
    // 当前可用的USB串口名称列表
    let available_usb_ports: Vec<String> = ports.iter()
        .filter_map(|p| {
            if let SerialPortType::UsbPort(_) = &p.port_type {
                Some(p.port_name.clone())
            } else {
                None
            }
        })
        .collect();
    
    // 第一步：通过串号识别传统的USBSCR设备
    for p in ports.iter() {
        #[cfg(not(windows))]
        info!("USB Serial 设备:{:?}", p);
        match p.port_type.clone(){
            SerialPortType::UsbPort(port) => {
                let serial_number = port.serial_number.unwrap_or("".to_string());
                if serial_number.starts_with("USBSCR"){
                    let port_name = p.port_name.clone();
                    let (width, height) = get_screen_size_from_serial_number(&serial_number);
                    devices.push(UsbScreenInfo{
                        label: format!("USB {}", &port_name), 
                        address: port_name.clone(),
                        width,
                        height,
                        is_esp32_wifi: false,
                    });
                    identified_ports.push(port_name);
                }
            }
            _ => ()
        }
    }
    
    // 第二步：对未识别的USB串口设备进行ReadInfo探测
    for p in ports.iter() {
        // 跳过已识别的端口
        if identified_ports.contains(&p.port_name) {
            continue;
        }
        
        // 只探测USB类型的串口
        if let SerialPortType::UsbPort(_) = &p.port_type {
            debug!("尝试ReadInfo探测: {}", p.port_name);
            if let Some((width, height)) = probe_port_with_readinfo(&p.port_name, 800) {
                info!("通过ReadInfo发现ESP32 WiFi屏幕: {} ({}x{})", p.port_name, width, height);
                // 缓存发现的设备信息
                if let Ok(mut cache) = ESP32_DEVICE_CACHE.lock() {
                    cache.insert(p.port_name.clone(), (width, height));
                }
                devices.push(UsbScreenInfo{
                    label: format!("ESP32 {}", &p.port_name),
                    address: p.port_name.clone(),
                    width,
                    height,
                    is_esp32_wifi: true,
                });
                identified_ports.push(p.port_name.clone());
            }
        }
    }
    
    // 第三步：从缓存中恢复已发现但当前无法探测的ESP32设备（可能正在被使用）
    if let Ok(cache) = ESP32_DEVICE_CACHE.lock() {
        for (port_name, (width, height)) in cache.iter() {
            // 如果端口仍在系统中可见，但未被探测到（可能被占用），从缓存恢复
            if available_usb_ports.contains(port_name) && !identified_ports.contains(port_name) {
                debug!("从缓存恢复ESP32设备: {} ({}x{})", port_name, width, height);
                devices.push(UsbScreenInfo{
                    label: format!("ESP32 {}", port_name),
                    address: port_name.clone(),
                    width: *width,
                    height: *height,
                    is_esp32_wifi: true,
                });
            }
        }
    }
    
    devices
}

pub fn clear_screen(color: Rgb<u8>, interface:&Interface, width: u16, height: u16) -> anyhow::Result<()>{
    let mut img = RgbImage::new(width as u32, height as u32);
    for p in img.pixels_mut(){
        *p = color;
    }
    draw_rgb_image(0, 0, &img, interface)
}

#[cfg(feature = "usb-serial")]
pub fn clear_screen_serial(color: Rgb<u8>, port:&mut dyn SerialPort, width: u16, height: u16) -> anyhow::Result<()>{
    let mut img = RgbImage::new(width as u32, height as u32);
    for p in img.pixels_mut(){
        *p = color;
    }
    draw_rgb_image_serial(0, 0, &img, port)
}

pub fn draw_rgb_image(x: u16, y: u16, img:&RgbImage, interface:&Interface) -> anyhow::Result<()>{
    //ST7789驱动使用的是Big-Endian
    let rgb565 = rgb888_to_rgb565_be(&img, img.width() as usize, img.height() as usize);
    draw_rgb565(&rgb565, x, y, img.width() as u16, img.height() as u16, interface)
}

pub fn draw_rgb565(rgb565:&[u8], x: u16, y: u16, width: u16, height: u16, interface:&Interface) -> anyhow::Result<()>{
    // info!("压缩前大小:{}", rgb565.len());
    let rgb565_u8_slice = lz4_flex::compress_prepend_size(rgb565);
    // info!("压缩后大小:{}", rgb565_u8_slice.len());
    if rgb565_u8_slice.len() >1024*28 {
        return Err(anyhow!("图像太大了!"));
    }
    const IMAGE_AA:u64 = 7596835243154170209;
    const BOOT_USB:u64 = 7093010483740242786;
    const IMAGE_BB:u64 = 7596835243154170466;

    let img_begin = &mut [0u8; 16];
    img_begin[0..8].copy_from_slice(&IMAGE_AA.to_be_bytes());
    img_begin[8..10].copy_from_slice(&width.to_be_bytes());
    img_begin[10..12].copy_from_slice(&height.to_be_bytes());
    img_begin[12..14].copy_from_slice(&x.to_be_bytes());
    img_begin[14..16].copy_from_slice(&y.to_be_bytes());
    // info!("绘制:{x}x{y} {width}x{height}");
    // block_on(interface.bulk_out(BULK_OUT_EP, img_begin.into())).status?;
    block_on(async {
        async_std::future::timeout(Duration::from_millis(100), interface.bulk_out(BULK_OUT_EP, img_begin.into()))
            .await
    })?.status?;
    //读取
    // let result = block_on(interface.bulk_in(BULK_IN_EP, RequestBuffer::new(64))).data;
    // let msg = String::from_utf8(result)?;
    // println!("{msg}ms");
    // block_on(interface.bulk_out(BULK_OUT_EP, rgb565_u8_slice.into())).status?;
    block_on(async {
        async_std::future::timeout(Duration::from_millis(100), interface.bulk_out(BULK_OUT_EP, rgb565_u8_slice.into()))
            .await
    })?.status?;
    // block_on(interface.bulk_out(BULK_OUT_EP, IMAGE_BB.to_be_bytes().into())).status?;
    block_on(async {
        async_std::future::timeout(Duration::from_millis(100), interface.bulk_out(BULK_OUT_EP, IMAGE_BB.to_be_bytes().into()))
            .await
    })?.status?;
    // info!("绘制成功..");
    Ok(())
}

#[cfg(feature = "usb-serial")]
pub fn draw_rgb_image_serial(x: u16, y: u16, img:&RgbImage, port:&mut dyn SerialPort) -> anyhow::Result<()>{
    //ST7789驱动使用的是Big-Endian
    let rgb565 = rgb888_to_rgb565_be(&img, img.width() as usize, img.height() as usize);
    draw_rgb565_serial(&rgb565, x, y, img.width() as u16, img.height() as u16, port)
}

// 老设备使用分段发送方式（兼容性更好）
#[cfg(feature = "usb-serial")]
pub fn draw_rgb565_serial_legacy(rgb565:&[u8], x: u16, y: u16, width: u16, height: u16, port:&mut dyn SerialPort) -> anyhow::Result<()>{
    let compressed = lz4_flex::compress_prepend_size(rgb565);

    const IMAGE_AA:u64 = 7596835243154170209;
    const IMAGE_BB:u64 = 7596835243154170466;

    let mut header = [0u8; 16];
    header[0..8].copy_from_slice(&IMAGE_AA.to_be_bytes());
    header[8..10].copy_from_slice(&width.to_be_bytes());
    header[10..12].copy_from_slice(&height.to_be_bytes());
    header[12..14].copy_from_slice(&x.to_be_bytes());
    header[14..16].copy_from_slice(&y.to_be_bytes());
    
    // 分段发送，兼容老设备
    port.write_all(&header)?;
    port.flush()?;
    port.write_all(&compressed)?;
    port.flush()?;
    port.write_all(&IMAGE_BB.to_be_bytes())?;
    port.flush()?;
    Ok(())
}

// 320x240屏幕连接到usb，然后在编辑器中一边添加多张gif，一边保存时，有时候rp2040会死机，同时编辑器也会卡死。
//第一：首先解决usb死机后，软件卡死问题
//第二：找到硬件代码死机问题，增加判断逻辑

#[cfg(feature = "usb-serial")]
pub fn draw_rgb565_serial(rgb565:&[u8], x: u16, y: u16, width: u16, height: u16, port:&mut dyn SerialPort) -> anyhow::Result<()>{
    
    let compressed = lz4_flex::compress_prepend_size(rgb565);

    const IMAGE_AA:u64 = 7596835243154170209;
    const IMAGE_BB:u64 = 7596835243154170466;

    // 构建帧头
    let mut header = [0u8; 16];
    header[0..8].copy_from_slice(&IMAGE_AA.to_be_bytes());
    header[8..10].copy_from_slice(&width.to_be_bytes());
    header[10..12].copy_from_slice(&height.to_be_bytes());
    header[12..14].copy_from_slice(&x.to_be_bytes());
    header[14..16].copy_from_slice(&y.to_be_bytes());

    // 将帧头 + 压缩数据 + 帧尾合并成一个完整帧发送，减少系统调用和flush次数
    let mut frame = Vec::with_capacity(header.len() + compressed.len() + 8);
    frame.extend_from_slice(&header);
    frame.extend_from_slice(&compressed);
    frame.extend_from_slice(&IMAGE_BB.to_be_bytes());
    
    // 一次性写入完整帧
    port.write_all(&frame)?;
    port.flush()?;
    
    Ok(())
}

#[cfg(not(windows))]
fn list_acm_devices() -> Vec<String> {
    let dir_path = std::path::Path::new("/dev");
    let entries = match std::fs::read_dir(dir_path){
        Err(err) => {
            log::error!("error list /dev/ {:?}", err);
            return vec![];
        }
        Ok(e) => e
    };
    entries.filter_map(|entry| {
        entry.ok().and_then(|e| {
            let path = e.path();
            if let Some(file_name) = path.file_name() {
                if let Some(name) = file_name.to_str() {
                    if name.starts_with("ttyACM") {
                        return Some(format!("/dev/{name}"));
                    }
                }
            }
            None
        })
    }).collect()
}
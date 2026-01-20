use std::{net::TcpStream, sync::Mutex, time::{Duration, Instant}};

use crossbeam_channel::{bounded, Receiver, Sender};
use fast_image_resize::{images::Image, Resizer};
use image::{buffer::ConvertBuffer, RgbImage, RgbaImage};
use log::info;
use once_cell::sync::Lazy;
use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use tungstenite::{connect, stream::MaybeTlsStream, WebSocket};

use crate::rgb565::rgb888_to_rgb565_be;

// WiFi帧差分协议 Magic Numbers (8字节)
// 格式: MAGIC(8) + WIDTH(2) + HEIGHT(2) + LZ4_COMPRESSED_DATA
const WIFI_KEY_MAGIC: &[u8; 8] = b"wflz4ke_"; // lz4压缩的关键帧(完整RGB565)
const WIFI_DLT_MAGIC: &[u8; 8] = b"wflz4dl_"; // lz4压缩的差分帧(XOR差分数据)
const WIFI_NOP_MAGIC: &[u8; 8] = b"wflz4no_"; // 无变化帧(屏幕静止，跳过绘制)

// 无变化帧阈值：压缩后小于此大小认为画面没变化
const NO_CHANGE_THRESHOLD: usize = 200;

// WiFi帧差分编码器
struct DeltaEncoder {
    prev_frame: Vec<u8>,       // 上一帧RGB565数据
    frame_count: u32,          // 帧计数
    key_frame_interval: u32,   // 关键帧间隔(默认60帧)
}

impl DeltaEncoder {
    fn new(key_frame_interval: u32) -> Self {
        Self {
            prev_frame: Vec::new(),
            frame_count: 0,
            key_frame_interval,
        }
    }

    // 编码一帧RGB565数据
    // 返回: (编码后的数据, 帧类型描述)
    fn encode(&mut self, rgb565_data: &[u8], width: u16, height: u16) -> (Vec<u8>, &'static str) {
        let need_key_frame = self.prev_frame.len() != rgb565_data.len()
            || self.frame_count == 0
            || self.frame_count % self.key_frame_interval == 0;

        if need_key_frame {
            // 关键帧: 直接压缩完整数据
            let compressed = lz4_flex::compress_prepend_size(rgb565_data);
            
            // 构建帧数据: MAGIC + WIDTH + HEIGHT + COMPRESSED_DATA
            let mut frame = Vec::with_capacity(12 + compressed.len());
            frame.extend_from_slice(WIFI_KEY_MAGIC);
            frame.extend_from_slice(&width.to_be_bytes());
            frame.extend_from_slice(&height.to_be_bytes());
            frame.extend_from_slice(&compressed);
            
            // 保存当前帧作为参考帧
            self.prev_frame = rgb565_data.to_vec();
            self.frame_count = self.frame_count.wrapping_add(1);
            
            (frame, "KEY")
        } else {
            // 差分帧: 计算XOR差分并压缩
            let delta: Vec<u8> = rgb565_data.iter()
                .zip(self.prev_frame.iter())
                .map(|(curr, prev)| curr ^ prev)
                .collect();
            
            let compressed_delta = lz4_flex::compress_prepend_size(&delta);
            
            // 如果压缩后数据很小，说明画面几乎没变化，发送无变化帧
            if compressed_delta.len() < NO_CHANGE_THRESHOLD {
                let mut frame = Vec::with_capacity(12);
                frame.extend_from_slice(WIFI_NOP_MAGIC);
                frame.extend_from_slice(&width.to_be_bytes());
                frame.extend_from_slice(&height.to_be_bytes());
                
                self.frame_count = self.frame_count.wrapping_add(1);
                
                (frame, "NOP")
            } else {
                let compressed_key = lz4_flex::compress_prepend_size(rgb565_data);
                
                // 如果差分帧比关键帧还大，使用关键帧
                if compressed_delta.len() >= compressed_key.len() {
                    let mut frame = Vec::with_capacity(12 + compressed_key.len());
                    frame.extend_from_slice(WIFI_KEY_MAGIC);
                    frame.extend_from_slice(&width.to_be_bytes());
                    frame.extend_from_slice(&height.to_be_bytes());
                    frame.extend_from_slice(&compressed_key);
                    
                    self.prev_frame = rgb565_data.to_vec();
                    self.frame_count = self.frame_count.wrapping_add(1);
                    
                    (frame, "KEY")
                } else {
                    // 使用差分帧
                    let mut frame = Vec::with_capacity(12 + compressed_delta.len());
                    frame.extend_from_slice(WIFI_DLT_MAGIC);
                    frame.extend_from_slice(&width.to_be_bytes());
                    frame.extend_from_slice(&height.to_be_bytes());
                    frame.extend_from_slice(&compressed_delta);
                    
                    // 更新参考帧
                    self.prev_frame = rgb565_data.to_vec();
                    self.frame_count = self.frame_count.wrapping_add(1);
                    
                    (frame, "DLT")
                }
            }
        }
    }

    // 重置编码器状态
    fn reset(&mut self) {
        self.prev_frame.clear();
        self.frame_count = 0;
    }
}

#[derive(Serialize, Deserialize, Debug)]
struct DisplayConfig{
    display_type: Option<String>,
    rotated_width: u32,
    rotated_height: u32
}

pub enum Message{
    Connect(String),
    Disconnect,
    Image(RgbaImage)
}

#[derive(Debug, Clone)]
pub struct StatusInfo{
    pub ip: Option<String>,
    pub status: Status,
    pub delay_ms: u64,
}

#[derive(Debug, Clone)]
pub enum Status{
    NotConnected,
    Connected,
    ConnectFail,
    Disconnected,
    Connecting,
}

impl Status{
    pub fn name(&self) -> &str{
        match self{
            Status::NotConnected => "未连接",
            Status::Connected => "连接成功",
            Status::ConnectFail => "连接失败",
            Status::Disconnected => "连接断开",
            Status::Connecting => "正在连接",
        }
    }
}

static CONFIG: Lazy<Mutex<(StatusInfo, Sender<Message>)>> = Lazy::new(|| {
    let (sender, recv) = bounded(1);
    let _ = std::thread::spawn(move ||{
        start(recv);
    });
    Mutex::new((StatusInfo{
        ip: None,
        status: Status::NotConnected,
        delay_ms: 1,
    }, sender))
});

fn set_status(ip: Option<String>, status: Status) -> Result<()>{
    let mut config = CONFIG.lock().map_err(|err| anyhow!("{err:?}"))?;
    config.0.status = status;
    config.0.ip = ip;
    Ok(())
}

pub fn set_delay_ms(delay_ms: u64) -> Result<()>{
    let mut config = CONFIG.lock().map_err(|err| anyhow!("{err:?}"))?;
    config.0.delay_ms = delay_ms;
    Ok(())
}

pub fn send_message(msg: Message) -> Result<()>{
    let sender = {
        let config = CONFIG.lock().map_err(|err| anyhow!("{err:?}"))?;
        let s = config.1.clone();
        drop(config);
        s
    };
    sender.send(msg)?;
    Ok(())
}

pub fn try_send_message(msg: Message) -> Result<()>{
    let config = CONFIG.lock().map_err(|err| anyhow!("{err:?}"))?;
    config.1.try_send(msg)?;
    Ok(())
}

pub fn get_status() -> Result<StatusInfo>{
    let config = CONFIG.lock().map_err(|err| anyhow!("{err:?}"))?;
    Ok(config.0.clone())
}

fn get_display_config(ip: &str) -> Result<DisplayConfig>{
    let resp = reqwest::blocking::Client::builder()
    .timeout(Duration::from_secs(2))
    .build()?
    .get(&format!("http://{ip}/display_config"))
    .send()?
    .json::<DisplayConfig>()?;
    Ok(resp)
}

fn start(receiver: Receiver<Message>){
    let mut socket: Option<WebSocket<MaybeTlsStream<TcpStream>>> = None;
    let mut screen_ip = String::new();
    let mut delta_encoder = DeltaEncoder::new(60);

    println!("启动upload线程(差分协议+ACK)...");

    let mut display_config = None;
    let mut connected = false;
    
    loop{
        match receiver.recv(){
            Ok(msg) => {
                match msg{
                    Message::Disconnect => {
                        screen_ip = String::new();
                        delta_encoder.reset();
                        if let Ok(mut cfg) = CONFIG.lock(){
                            cfg.0.status = Status::Disconnected
                        }
                        if let Some(mut s) = socket.take(){
                            let _ = s.close(None);
                        }
                    }
                    Message::Connect(ip) => {
                        screen_ip = ip.clone();
                        delta_encoder.reset();
                        if let Ok(cfg) = get_display_config(&ip){
                            display_config = Some(cfg);
                        }else{
                            eprintln!("display config获取失败!");
                        }
                        println!("接收到 serverIP...");
                        connected = connect_socket(ip, &mut socket).is_ok();
                    }
                    Message::Image(mut image) => {
                        let delay_ms = {
                            if let Ok(mut cfg) = CONFIG.try_lock(){
                                cfg.0.status = if connected{
                                    Status::Connected
                                }else{
                                    Status::Disconnected
                                };
                                let v = cfg.0.delay_ms;
                                drop(cfg);
                                v
                            }else{
                                1
                            }
                        };
                        if display_config.is_none(){
                            match get_display_config(&screen_ip){
                                Ok(cfg) => {
                                    display_config = Some(cfg);
                                }
                                Err(_err) => {
                                    eprintln!("Message::Image display config获取失败!");
                                    std::thread::sleep(Duration::from_secs(3));
                                    let screen_ip_clone = screen_ip.clone();
                                    std::thread::spawn(move ||{
                                        let r = send_message(Message::Connect(screen_ip_clone));
                                        println!("重新连接 SetIp {r:?}...");
                                    });
                                }
                            }
                        }
                        let (dst_width, dst_height) = match display_config.as_ref(){
                            Some(c) => (c.rotated_width, c.rotated_height),
                            None => continue,
                        };

                        if let Some(s) = socket.as_mut(){
                            if s.can_write(){
                                connected = true;
                            }
                        }
                        if connected{
                            if let Some(s) = socket.as_mut(){
                                let t1 = Instant::now();
                                
                                // 缩放图像
                                let img = match fast_resize(&mut image, dst_width, dst_height){
                                    Ok(v) => v,
                                    Err(err) => {
                                        eprintln!("图片压缩失败:{}", err.root_cause());
                                        continue;
                                    }
                                };
                                
                                // 转换为RGB565
                                let rgb565 = rgb888_to_rgb565_be(img.as_raw(), img.width() as usize, img.height() as usize);
                                
                                // 使用差分编码
                                let (out, frame_type) = delta_encoder.encode(&rgb565, dst_width as u16, dst_height as u16);
                                let encode_ms = t1.elapsed().as_millis();
                                
                                // 设置读取超时3秒
                                if let tungstenite::stream::MaybeTlsStream::Plain(stream) = s.get_mut() {
                                    let _ = stream.set_read_timeout(Some(Duration::from_secs(3)));
                                }
                                
                                // 发送帧
                                let send_start = Instant::now();
                                let ret1 = s.write(tungstenite::Message::Binary(out.clone().into()));
                                let ret2 = s.flush();
                                
                                if ret1.is_err() || ret2.is_err(){
                                    info!("ws write:{ret1:?}");
                                    info!("ws flush:{ret2:?}");
                                    connected = false;
                                    delta_encoder.reset();
                                    let _ = socket.take();
                                    continue;
                                }
                                
                                // 等待ACK/NACK (3秒超时)
                                match s.read() {
                                    Ok(msg) => {
                                        match msg {
                                            tungstenite::Message::Text(text) => {
                                                if text == "NACK" {
                                                    println!("收到NACK，重置编码器");
                                                    delta_encoder.reset();
                                                }
                                                // ACK则继续
                                            }
                                            tungstenite::Message::Close(_) => {
                                                connected = false;
                                                delta_encoder.reset();
                                                let _ = socket.take();
                                                continue;
                                            }
                                            _ => {}
                                        }
                                    }
                                    Err(e) => {
                                        eprintln!("等待ACK超时/失败: {}，重置编码器", e);
                                        delta_encoder.reset();
                                    }
                                }
                                
                                let send_ms = send_start.elapsed().as_millis();
                                let total_ms = t1.elapsed().as_millis();
                                println!("[FRAME] type={} {}x{} bytes={} encode={}ms send+ack={}ms total={}ms", 
                                    frame_type, img.width(), img.height(), out.len(), encode_ms, send_ms, total_ms);
                                
                                if delay_ms > 0 {
                                    std::thread::sleep(Duration::from_millis(delay_ms));
                                }
                            }
                        }else{
                            if let Some(mut s) = socket.take(){
                                let _ = s.close(None);
                            }
                            delta_encoder.reset();
                            let _ = set_status(None, Status::Disconnected);
                            println!("连接断开 3秒后重连:{screen_ip}");
                            if screen_ip.len() > 0{
                                std::thread::sleep(Duration::from_secs(3));
                                let screen_ip_clone = screen_ip.clone();
                                std::thread::spawn(move ||{
                                    let r = send_message(Message::Connect(screen_ip_clone));
                                    println!("重新连接 SetIp {r:?}...");
                                });
                            }
                        }
                    }
                }
            }
            Err(_err) => {
                std::thread::sleep(Duration::from_millis(10));
            }
        }
    }
}

fn connect_socket(ip: String, old_socket: &mut Option<WebSocket<MaybeTlsStream<TcpStream>>>) -> Result<()>{
    if let Some(mut s) = old_socket.take(){
        let _ = s.close(None);
    }
    let _ = set_status(Some(ip.clone()), Status::Connecting);
    let url = format!("ws://{ip}/ws");
    println!("开始连接:{url}");
    if let Ok((s, _resp)) = connect(url){
        *old_socket = Some(s);
        let ret = set_status(None, Status::Connected);
        println!("连接成功{ip}.. 设置状态:{ret:?}");
    }else{
        println!("连接失败{ip}..");
        let _ = set_status(None, Status::ConnectFail);
    }
    Ok(())
}

fn fast_resize(src: &mut RgbaImage, dst_width: u32, dst_height: u32) -> Result<RgbImage>{
    let mut dst_image = Image::new(
        dst_width,
        dst_height,
        fast_image_resize::PixelType::U8x3,
    );
    let mut src:RgbImage = src.convert();
    if src.width() != dst_width || src.height() != dst_height{
        let v = Image::from_slice_u8(src.width(), src.height(), src.as_mut(), fast_image_resize::PixelType::U8x3)?;
        let mut resizer = Resizer::new();
        resizer.resize(&v, &mut dst_image, None)?;
        Ok(RgbImage::from_raw(dst_image.width(), dst_image.height(), dst_image.buffer().to_vec()).unwrap())
    }else{
        Ok(src.convert())
    }
}

// 获取wifi屏幕参数，测试是否可以连接成功
pub fn test_screen_sync(ip: String) -> Result<()>{
    let resp = reqwest::blocking::get(&format!("http://{ip}/display_config"))?
        .json::<DisplayConfig>()?;
    println!("屏幕大小:{}x{}", resp.rotated_width, resp.rotated_height);
    // 显示hello
    let json = r#"[{"Rectangle":{"fill_color":"black","height":240,"width":240,"stroke_width":0,"left":0,"top":0}},{"Text":{"color":"white","size":20,"text":"Hello!","x":10,"y":15}},{"Text":{"color":"white","size":20,"text":"USB Screen","x":10,"y":40}}]"#;
    let _resp = reqwest::blocking::Client::new()
        .post(&format!("http://{ip}/draw_canvas"))
        .body(json.as_bytes())
        .send()?
        .text()?;
    Ok(())
}

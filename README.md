# USB Screen
 USB屏幕&编辑器

 ![capture](images/capture.png)

# 图文教程:

# [https://zhuanlan.zhihu.com/p/698789562](https://zhuanlan.zhihu.com/p/698789562)

# 视频教程
# [https://www.bilibili.com/video/BV1eTTwe6EFU/?vd_source=a2700de3db7bd5f0117df32bdd5cef9f](https://www.bilibili.com/video/BV1eTTwe6EFU/?vd_source=a2700de3db7bd5f0117df32bdd5cef9f)

# 硬件

本项目支持两种硬件方案：

| 方案 | 主控芯片 | 连接方式 | 支持屏幕型号 | 特点 |
|------|---------|---------|-------------|------|
| **RP2040** | RP2040 | USB | ST7735、ST7789 (2种) | 成本低、稳定 |
| **ESP32 S2/S3** | ESP32-S2/S3 | USB串口 / WiFi | ST7735S、ST7789、ST7796 (6种) | 屏幕型号多、支持WiFi |

---

## 方案一：RP2040 USB屏幕

### 支持的屏幕型号

支持 ST7735 128x160 和 ST7789 320x240 两种屏幕

### ST7735接线方式
```
    GND <=> GND
    VCC <=> 3V3
    SCL <=> SCLK(GPIO6)
    SDA <=> MOSI(GPIO7)
    RES <=> RST(GPIO14)
    DC  <=> DC(GPIO13)
    CS  <=> GND
    BLK <=> 不连接
```
![rp2040.png](images/rp2040.png)
![st7735.jpg](images/st7735.png)

### ST7789接线方式
```
    GND   <=> GND
    VCC   <=> 3V3
    SCL   <=> PIN6(clk)
    SDA   <=> PIN7(mosi)
    RESET <=> PIN14(rst)
    AO    <=> PIN13
    CS    <=> PIN9
    BL    <=> 5V
```
![st7789.jpg](images/st7789.png)

### ST7789 240x240 接线方式
```
    GND   <=> GND
    VCC   <=> 3V3
    SCL   <=> PIN6(clk)
    SDA   <=> PIN7(mosi)
    RESET <=> PIN14(rst)
    DC    <=> PIN13
    CS    <=> PIN9
    BL    <=> 5V
```

### 固件源码
https://github.com/planet0104/rp2040_usb_screen

---

## 方案二：ESP32 S2/S3 WiFi/USB屏幕

基于 **ESP32-S2 / ESP32-S3** 的屏幕方案，内存更大，支持更多屏幕型号，可通过 **USB串口** 或 **WiFi** 连接。

### 支持的屏幕型号

| 屏幕型号 | 分辨率 | 是否需要CS引脚 |
|---------|--------|--------------|
| ST7735S | 80x160 | 需要 |
| ST7735S | 128x160 | 需要 |
| ST7789 | 240x240 | 不需要 |
| ST7789 | 240x320 | 需要 |
| ST7789V | 135x240 | 需要 |
| ST7796 | 320x480 | 需要 |

### 硬件要求

- ESP32-S2 或 ESP32-S3 开发板（带 PSRAM，建议 4MB Flash + 2MB PSRAM）
- 支持的 TFT 显示屏（见上表）
- USB 数据线

### 通用引脚定义

| 显示屏引脚 | ESP32 引脚 | 说明 |
|-----------|------------|------|
| GND | GND | 接地 |
| VCC | 3V3 | 电源 3.3V |
| SCL/CLK | GPIO6 | SPI 时钟 |
| SDA/MOSI | GPIO7 | SPI 数据 |
| RST/RES | GPIO8 | 复位 |
| DC/AO | GPIO5 | 数据/命令选择 |
| CS | GPIO4 | 片选（部分屏幕需要） |
| BL/BLK | 悬空或 VBUS | 背光（可接 VBUS 5V） |

### 连接方式

ESP32 屏幕支持两种连接方式：

1. **USB串口模式**：通过 USB 数据线连接，软件会自动识别 ESP32 屏幕设备
2. **WiFi模式**：在编辑器中配置屏幕的 IP 地址即可连接

### 固件烧录与配置

ESP32 屏幕的固件烧录、WiFi配置、屏幕参数设置等详细说明，请参考 ESP32 WiFi Screen 项目文档：

**固件源码与文档**：https://github.com/planet0104/esp32-wifi-screen

#### 快速烧录步骤

1. 让开发板进入烧录模式：按住 Boot 并上电/复位
2. 使用 `esptool.exe` 烧录预编译固件：

**ESP32-S2：**
```powershell
.\esptool.exe -p COM6 --chip esp32s2 write_flash 0x0 esp32-wifi-screen-esp32s2-merged.bin
```

**ESP32-S3：**
```powershell
.\esptool.exe -p COM6 --chip esp32s3 write_flash 0x0 esp32-wifi-screen-esp32s3-merged.bin
```

3. 烧录后连接设备热点（如 `ESP32-WiFiScreen`），访问 `http://192.168.72.1` 配置屏幕参数和 WiFi

### 在 USB-Screen 编辑器中使用

1. **USB串口模式**：直接用 USB 线连接 ESP32 开发板，编辑器会自动发现并列出设备
2. **WiFi模式**：在编辑器中输入屏幕的局域网 IP 地址即可连接

---


# 编译

## 编译aarch64-linux

1、设置default features，启用 v4l-webcam

```toml
[features]
default = ["v4l-webcam", "usb-serial"]
```

2、启动 DockerDesktop

3、进入 wsl2 Ubuntu

4、安装 cross

```shell
cargo install cross --git https://github.com/cross-rs/cross
```

5、编译

注意 Cross.toml 中的配置

```shell
# rustup component add rust-src --toolchain nightly
RUSTFLAGS="-Zlocation-detail=none" cross +nightly build -Z build-std=std,panic_abort \
  -Z build-std-features=panic_immediate_abort \
  -Z build-std-features="optimize_for_size" \
  --target aarch64-unknown-linux-gnu --release
```

# 运行编辑器

## windows中运行

设置 deault features

```toml
[features]
default = ["editor", "tray", "nokhwa-webcam"]
```

```cmd
./run.cmd
```

## Ubuntu中运行

设置 deault features

```toml
[features]
default = ["editor", "v4l-webcam"]
```

```bash
# export https_proxy=http://192.168.1.25:6003;export http_proxy=http://192.168.1.25:6003;export all_proxy=socks5://192.168.1.25:6003
# export https_proxy=;export http_proxy=;export all_proxy=;
sudo apt-get install -y libclang-dev libv4l-dev libudev-dev

sh run.sh
# sudo ./target/debug/USB-Screen
# sudo ./target/debug/USB-Screen editor

## v4l utils
## sudo apt install v4l-utils
## v4l2-ctl  --list-formats -d /dev/video0
## v4l2-ctl --list-formats-ext -d /dev/video0
```

## 飞牛私有云 fnOS 编译

```bash
# 切换到root模式(登录 planet,root123)
sudo -i
# 首先安装rust
# ...
# 飞牛OS编译前需要升级libc6=2.36-9+deb12u9
sudo apt-get install aptitude
aptitude install libc6=2.36-9+deb12u9
apt install build-essential
#安装依赖库
apt install pkg-config
sudo apt-get install -y libclang-dev libv4l-dev libudev-dev
# 打开x86_64 linux编译特征
# ！！注意关闭 editor特征！！
# x86_64 linux
# default = ["v4l-webcam", "usb-serial"]
# 克隆然后编译
rm Cargo.lock
cargo build --release
```
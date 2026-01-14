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

所有编译脚本都使用 `cargo zbuild` 进行交叉编译，自动指定 features，**无需手动修改 Cargo.toml**。

## 前置要求

1. 安装 Rust: https://rustup.rs
2. 安装 cargo-zbuild: `cargo install cargo-zbuild`
3. 安装 Docker Desktop (交叉编译需要)

## 编译脚本一览

| 脚本 | 目标平台 | Features | 说明 |
|------|----------|----------|------|
| `build-x86_64_windows.cmd` | Windows x64 | editor, tray, nokhwa-webcam, usb-serial | Windows 桌面版 |
| `build-x86_64_linux.cmd` | x86_64 Linux (gnu) | editor, v4l-webcam, usb-serial | Linux 桌面版 (默认带 editor) |
| `build-x86_64_linux.cmd no-editor` | x86_64 Linux (gnu) | v4l-webcam, usb-serial | Linux 无桌面版 |
| `build-x86_64_linux_musl.cmd` | x86_64 Linux (musl) | usb-serial | 静态链接版本，兼容性好 |
| `build-aarch64-musl.cmd` | OpenWrt ARM64 | v4l-webcam, usb-serial | 路由器/嵌入式设备 |

## Windows 编译

```cmd
:: 需要以管理员身份运行 (读取硬件信息)
.\build-x86_64_windows.cmd
```

输出文件: `target/x86_64-pc-windows-msvc/release/USB-Screen.exe`

## x86_64 Linux 编译 (带 editor)

```cmd
:: 需要启动 Docker Desktop
.\build-x86_64_linux.cmd
```

输出文件: `target/x86_64-unknown-linux-gnu/release/USB-Screen`

## x86_64 Linux 编译 (无 editor)

```cmd
:: 需要启动 Docker Desktop
.\build-x86_64_linux.cmd no-editor
```

## OpenWrt ARM64 (aarch64) 编译

```cmd
:: 需要启动 Docker Desktop
.\build-aarch64-musl.cmd
```

输出文件: `target/aarch64-unknown-linux-musl/release/USB-Screen`

## 飞牛私有云 fnOS 编译

飞牛 fnOS 推荐使用 musl 静态链接版本，兼容性更好：

```cmd
:: 需要启动 Docker Desktop
.\build-x86_64_linux_musl.cmd
```

输出文件: `target/x86_64-unknown-linux-musl/release/USB-Screen`

如果需要 v4l 摄像头功能，使用 gnu 版本：

```cmd
.\build-x86_64_linux.cmd no-editor
```

# 运行编辑器

## Windows 中运行

```cmd
.\run.cmd
```

## Ubuntu 中运行

```bash
# 安装依赖
sudo apt-get install -y libclang-dev libv4l-dev libudev-dev

# 运行
sh run.sh

# v4l utils (可选，用于调试摄像头)
# sudo apt install v4l-utils
# v4l2-ctl --list-formats -d /dev/video0
# v4l2-ctl --list-formats-ext -d /dev/video0
```
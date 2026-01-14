@echo off
:: ============================================
:: OpenWrt ARM64 (aarch64-unknown-linux-musl) 交叉编译脚本
:: 需要启动 Docker Desktop
:: ============================================
::
:: 自动启用 features: v4l-webcam, usb-serial
:: 无需手动修改 Cargo.toml
::
:: 注意: 交叉编译使用 stable 工具链（nightly 在 Docker 中有兼容性问题）
:: 体积优化通过 Cargo.toml 的 profile.release 配置实现
::
:: ============================================

echo Checking Docker status...
docker info >nul 2>&1
if errorlevel 1 (
    echo Error: Docker is not running. Please start Docker Desktop first.
    exit /b 1
)

:: 安装目标工具链
echo Installing target toolchain...
rustup target add aarch64-unknown-linux-musl

:: 使用 cargo zbuild 交叉编译，指定 features
echo Building with v4l-webcam + usb-serial...
cargo zbuild --target aarch64-unknown-linux-musl --no-default-features --features "v4l-webcam,usb-serial"

echo.
echo ============================================
echo Build completed!
echo Output: target/aarch64-unknown-linux-musl/release/USB-Screen
echo ============================================

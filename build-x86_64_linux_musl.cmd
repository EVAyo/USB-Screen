@echo off
:: ============================================
:: x86_64 Linux (musl) 交叉编译脚本
:: 需要启动 Docker Desktop
:: ============================================
::
:: 使用 musl 静态链接，不依赖系统 glibc，兼容性更好
:: 适用于飞牛 fnOS 等系统
::
:: 用法:
::   build-x86_64_linux_musl.cmd            - 编译无 editor 版本 (默认)
::   build-x86_64_linux_musl.cmd editor     - 编译带 editor 版本
::
:: 注意: musl 版本不支持 v4l 摄像头功能
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
rustup target add x86_64-unknown-linux-musl

:: 根据参数决定 features
if /i "%1"=="editor" goto with_editor

echo Building with usb-serial...
cargo zbuild --target x86_64-unknown-linux-musl --no-default-features --features "usb-serial"
goto done

:with_editor
echo Building with editor + usb-serial...
cargo zbuild --target x86_64-unknown-linux-musl --no-default-features --features "editor,usb-serial"

:done
echo.
echo ============================================
echo Build completed!
echo Output: target/x86_64-unknown-linux-musl/release/USB-Screen
echo ============================================

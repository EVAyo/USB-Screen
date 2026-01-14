@echo off
:: ============================================
:: x86_64 Linux 交叉编译脚本
:: 需要启动 Docker Desktop
:: ============================================
::
:: 用法:
::   build-x86_64_linux.cmd            - 编译带 editor 版本 (默认)
::   build-x86_64_linux.cmd no-editor  - 编译无 editor 版本
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
rustup target add x86_64-unknown-linux-gnu

:: 根据参数决定 features
if /i "%1"=="no-editor" goto no_editor

echo Building with editor + v4l-webcam + usb-serial...
cargo zbuild --target x86_64-unknown-linux-gnu --no-default-features --features "editor,v4l-webcam,usb-serial"
goto done

:no_editor
echo Building with v4l-webcam + usb-serial...
cargo zbuild --target x86_64-unknown-linux-gnu --no-default-features --features "v4l-webcam,usb-serial"

:done
echo.
echo ============================================
echo Build completed!
echo Output: target/x86_64-unknown-linux-gnu/release/USB-Screen
echo ============================================

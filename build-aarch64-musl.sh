#!/bin/bash
# ============================================
# aarch64 Linux (musl) 交叉编译脚本 - WSL版本
# 在WSL子系统中运行，用于OpenWrt/ARM64设备
# ============================================
#
# 使用 musl 静态链接，不依赖系统 glibc，兼容性更好
# 适用于 OpenWrt、树莓派等 ARM64 设备
#
# 用法:
#   ./build-aarch64-musl.sh
#
# 首次运行需要安装依赖:
#   sudo apt update
#   sudo apt install -y build-essential pkg-config gcc-aarch64-linux-gnu
#
# ============================================

set -e

echo "============================================"
echo "USB-Screen aarch64 musl 编译脚本 (WSL版本)"
echo "============================================"

# 检查是否安装了交叉编译器
# 必须使用 musl 版本，gnu 版本会导致链接错误
AARCH64_CC=""
AARCH64_AR=""

if command -v aarch64-linux-musl-gcc &> /dev/null; then
    echo "使用 aarch64-linux-musl-gcc (musl工具链)"
    AARCH64_CC="aarch64-linux-musl-gcc"
    AARCH64_AR="aarch64-linux-musl-ar"
else
    echo "错误: 未找到 aarch64-linux-musl-gcc"
    echo ""
    echo "注意: 必须使用 musl 工具链，gnu 工具链 (gcc-aarch64-linux-gnu) 会导致链接错误!"
    echo ""
    echo "请安装 aarch64 musl 交叉编译工具链:"
    echo "  wget https://musl.cc/aarch64-linux-musl-cross.tgz"
    echo "  sudo tar -xzf aarch64-linux-musl-cross.tgz -C /opt/"
    echo "  echo 'export PATH=\"/opt/aarch64-linux-musl-cross/bin:\$PATH\"' >> ~/.bashrc"
    echo "  source ~/.bashrc"
    echo ""
    echo "然后重新运行此脚本"
    exit 1
fi

# 检查是否安装了 rustup
if ! command -v rustup &> /dev/null; then
    echo "错误: 未找到 rustup"
    echo "请先安装 Rust: curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh"
    exit 1
fi

# 安装目标工具链
echo "安装目标工具链..."
rustup target add aarch64-unknown-linux-musl

# 设置环境变量 - 强制完全静态链接
export CC_aarch64_unknown_linux_musl="$AARCH64_CC"
export AR_aarch64_unknown_linux_musl="$AARCH64_AR"
export CARGO_TARGET_AARCH64_UNKNOWN_LINUX_MUSL_LINKER="$AARCH64_CC"
# 关键: 强制静态链接C运行时，避免依赖libgcc_s.so
export RUSTFLAGS="-C target-feature=+crt-static -C link-self-contained=yes"

# 检测是否在 /mnt/ 下运行（Windows文件系统），如果是则使用WSL本地target目录避免权限问题
SCRIPT_DIR="$(pwd)"
if [[ "$SCRIPT_DIR" == /mnt/* ]]; then
    echo "检测到在Windows文件系统下运行，使用WSL本地target目录..."
    export CARGO_TARGET_DIR="$HOME/.cargo-target/USB-Screen"
    mkdir -p "$CARGO_TARGET_DIR"
    echo "Target目录: $CARGO_TARGET_DIR"
fi

echo "编译 v4l-webcam + usb-serial 版本..."
cargo build --release --target aarch64-unknown-linux-musl --no-default-features --features "v4l-webcam,usb-serial"

# 确定输出路径
if [ -n "$CARGO_TARGET_DIR" ]; then
    OUTPUT_FILE="$CARGO_TARGET_DIR/aarch64-unknown-linux-musl/release/USB-Screen"
else
    OUTPUT_FILE="target/aarch64-unknown-linux-musl/release/USB-Screen"
fi

# 检查编译结果
if [ $? -eq 0 ] && [ -f "$OUTPUT_FILE" ]; then
    echo ""
    echo "============================================"
    echo "编译成功!"
    echo "输出文件: $OUTPUT_FILE"
    echo ""
    # 显示文件信息
    ls -lh "$OUTPUT_FILE"
    echo ""
    # 检查文件类型
    echo "文件类型检查:"
    file "$OUTPUT_FILE"
    echo ""
    # 检查动态库依赖 (需要 qemu-user 或在目标设备上检查)
    echo "注意: 在x86_64上无法用ldd检查aarch64二进制文件"
    echo "请在目标ARM64设备上运行 'ldd USB-Screen' 检查依赖"
    echo "============================================"
    
    # 如果使用了WSL本地target，提示复制命令
    if [ -n "$CARGO_TARGET_DIR" ]; then
        echo ""
        echo "提示: 复制到项目目录:"
        echo "  cp \"$OUTPUT_FILE\" \"$SCRIPT_DIR/dist/aarch64-unknown-linux-musl/\""
    fi
else
    echo ""
    echo "============================================"
    echo "编译失败!"
    echo "============================================"
    exit 1
fi

#!/bin/bash
# ============================================
# x86_64 Linux (musl) 交叉编译脚本 - WSL版本
# 在WSL子系统中运行
# ============================================
#
# 使用 musl 静态链接，不依赖系统 glibc，兼容性更好
# 适用于飞牛 fnOS 等系统
#
# 用法:
#   ./build-x86_64_linux_musl.sh            - 编译无 editor 版本 (默认)
#   ./build-x86_64_linux_musl.sh editor     - 编译带 editor 版本
#
# 首次运行需要安装依赖:
#   sudo apt update
#   sudo apt install -y musl-tools build-essential pkg-config
#
# 注意: musl 版本不支持 v4l 摄像头功能
#
# ============================================

set -e

echo "============================================"
echo "USB-Screen musl 编译脚本 (WSL版本)"
echo "============================================"

# 检查是否安装了 musl-gcc
if ! command -v musl-gcc &> /dev/null; then
    echo "错误: 未找到 musl-gcc"
    echo "请先安装: sudo apt install musl-tools"
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
rustup target add x86_64-unknown-linux-musl

# 设置环境变量 - 强制完全静态链接
export CC_x86_64_unknown_linux_musl=musl-gcc
export AR_x86_64_unknown_linux_musl=ar
export CARGO_TARGET_X86_64_UNKNOWN_LINUX_MUSL_LINKER=musl-gcc
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

# 根据参数决定 features
if [ "$1" == "editor" ]; then
    echo "编译带 editor + usb-serial 版本..."
    cargo build --release --target x86_64-unknown-linux-musl --no-default-features --features "editor,usb-serial"
else
    echo "编译 usb-serial 版本..."
    cargo build --release --target x86_64-unknown-linux-musl --no-default-features --features "usb-serial"
fi

# 确定输出路径
if [ -n "$CARGO_TARGET_DIR" ]; then
    OUTPUT_FILE="$CARGO_TARGET_DIR/x86_64-unknown-linux-musl/release/USB-Screen"
else
    OUTPUT_FILE="target/x86_64-unknown-linux-musl/release/USB-Screen"
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
    # 检查是否静态链接
    echo "链接类型检查:"
    file "$OUTPUT_FILE"
    echo ""
    # 检查动态库依赖
    echo "动态库依赖检查 (应显示 statically linked 或 not a dynamic executable):"
    ldd "$OUTPUT_FILE" 2>&1 || true
    echo "============================================"
    
    # 如果使用了WSL本地target，提示复制命令
    if [ -n "$CARGO_TARGET_DIR" ]; then
        echo ""
        echo "提示: 复制到项目目录:"
        echo "  cp \"$OUTPUT_FILE\" \"$SCRIPT_DIR/dist/x86_64-unknown-linux-musl/\""
    fi
else
    echo ""
    echo "============================================"
    echo "编译失败!"
    echo "============================================"
    exit 1
fi

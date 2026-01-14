#!/bin/bash
# ============================================
# USB-Screen 运行脚本 (x86_64 musl版本)
# 适用于飞牛 fnOS 等Linux系统
# ============================================
#
# 用法:
#   bash run-x86_64_linux_musl.sh                    - 自动查找当前目录的.screen文件
#   bash run-x86_64_linux_musl.sh xxx.screen         - 指定screen配置文件
#
# ============================================

# 获取脚本所在目录
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROGRAM="$SCRIPT_DIR/target/x86_64-unknown-linux-musl/release/USB-Screen"

echo "============================================"
echo "USB-Screen 启动脚本"
echo "============================================"

# 检查程序是否存在
if [ ! -f "$PROGRAM" ]; then
    echo "错误: 程序文件不存在: $PROGRAM"
    echo "请先运行 ./build-x86_64_linux_musl.sh 编译"
    exit 1
fi

# 检查执行权限
if [ ! -x "$PROGRAM" ]; then
    echo "添加执行权限..."
    chmod +x "$PROGRAM"
fi

# 显示程序信息
echo "程序路径: $PROGRAM"
echo "文件信息: $(file "$PROGRAM" | cut -d: -f2)"
echo ""

# 如果传入了参数，使用参数作为screen文件
if [ -n "$1" ]; then
    SCREEN_FILE="$1"
    if [ ! -f "$SCREEN_FILE" ]; then
        echo "错误: 指定的screen文件不存在: $SCREEN_FILE"
        exit 1
    fi
    echo "使用screen文件: $SCREEN_FILE"
    echo "============================================"
    echo ""
    exec "$PROGRAM" "$SCREEN_FILE"
else
    # 查找当前目录下的.screen文件
    SCREEN_FILE=""
    for f in *.screen; do
        if [ -f "$f" ]; then
            SCREEN_FILE="$f"
            break
        fi
    done
    
    if [ -n "$SCREEN_FILE" ]; then
        echo "找到screen文件: $SCREEN_FILE"
        echo "============================================"
        echo ""
        exec "$PROGRAM" "$SCREEN_FILE"
    else
        echo "当前目录下未找到 .screen 文件"
        echo "可用的screen文件示例:"
        ls -1 "$SCRIPT_DIR"/*.screen 2>/dev/null || echo "  (项目目录下也没有找到)"
        echo ""
        echo "用法: bash $0 <screen文件路径>"
        exit 1
    fi
fi

#!/usr/bin/env sh
# 启动后端（Linux / macOS）。
# 用法：./run.sh [--release] [-- 程序参数]
set -e
cd "$(dirname "$0")"

if ! command -v cargo >/dev/null 2>&1; then
    echo "未找到 cargo。请先安装 Rust：curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh" >&2
    exit 1
fi

exec cargo run "$@"
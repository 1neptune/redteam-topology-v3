#!/bin/sh
# 容器入口：绑定挂载的宿主目录通常属于 root，先把它交给运行用户再降权启动。
set -e

if [ "$(id -u)" = "0" ]; then
    data_dir="${ATLAS_DATA_DIR:-/data}"
    mkdir -p "$data_dir"
    chown -R atlas:atlas "$data_dir" 2>/dev/null || true
    exec gosu atlas "$@"
fi

exec "$@"
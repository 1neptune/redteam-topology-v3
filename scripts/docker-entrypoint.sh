#!/bin/sh
# 容器入口：绑定挂载的宿主目录通常属于 root，先把它交给运行用户再降权启动。
#
# 启动信息一律打到 stderr，这样 `docker logs` / `docker compose logs` 一定能看到，
# 不必猜容器为什么起不来。
set -eu

log() { printf '[entrypoint] %s\n' "$*" >&2; }

log "uid=$(id -u) 参数个数=$# 命令=${*:-<空>}"

if [ "$#" -eq 0 ]; then
    log "没有收到要执行的命令：Dockerfile 的 CMD 丢失或被覆盖。"
    log "镜像需要一个可执行文件名，例如 CMD [\"redteam-atlas\"]。"
    exit 1
fi

if [ "$(id -u)" = "0" ]; then
    data_dir="${ATLAS_DATA_DIR:-/data}"
    mkdir -p "$data_dir"
    log "修正 $data_dir 属主为 atlas"
    # 目录里可能有很多文件，chown 失败不阻断启动，只记一条
    chown -R atlas:atlas "$data_dir" 2>/dev/null || log "警告：chown $data_dir 失败，继续以降权用户启动"

    if ! command -v gosu >/dev/null 2>&1; then
        log "gosu 不存在，无法降权，直接以 root 启动"
        exec "$@"
    fi

    log "降权到 atlas 并执行：$*"
    exec gosu atlas "$@"
fi

log "已是非 root 用户，直接执行：$*"
exec "$@"
# syntax=docker/dockerfile:1.7
#
# 构建参数（可在 .env 或 --build-arg 里覆盖，见 docker-compose.yml 的 build.args）
#   APT_MIRROR     Debian 包源。国内机器设成 mirrors.aliyun.com 之类能快十倍以上
#   NPM_REGISTRY   npm 源，同上
#   CARGO_MIRROR   crates.io 镜像，写成 sparse+https://... 形式；空则用官方源

ARG APT_MIRROR=""
ARG NPM_REGISTRY=""
ARG CARGO_MIRROR=""

# ---------- 前端构建 ----------
FROM node:22-alpine AS web
ARG NPM_REGISTRY=""
WORKDIR /build

# 只有设置了镜像源才切，没设置就走官方源
RUN if [ -n "$NPM_REGISTRY" ]; then npm config set registry "$NPM_REGISTRY"; fi

COPY frontend/package*.json ./
# npm 缓存挂进来：依赖没变时第二次构建不再下载
RUN --mount=type=cache,target=/root/.npm \
    npm ci --no-fund --no-audit

COPY frontend/ ./
RUN npm run build

# ---------- 后端构建 ----------
FROM rust:1-slim-bookworm AS api
ARG APT_MIRROR=""
ARG CARGO_MIRROR=""

# 切包源：把 deb.debian.org 换成镜像站，只改 sources.list
RUN if [ -n "$APT_MIRROR" ]; then \
        sed -i "s|deb.debian.org|$APT_MIRROR|g" /etc/apt/sources.list.d/debian.sources 2>/dev/null \
        || sed -i "s|deb.debian.org|$APT_MIRROR|g" /etc/apt/sources.list; \
    fi

# 只装编译 sqlite 需要的最小集合：bundled 模式调 cc 编 sqlite3.c，
# 不需要 build-essential 里的 make / dpkg-dev / perl，能省下上百 MB 下载
RUN --mount=type=cache,target=/var/cache/apt,sharing=locked \
    --mount=type=cache,target=/var/lib/apt,sharing=locked \
    apt-get update \
    && apt-get install -y --no-install-recommends gcc libc6-dev pkg-config \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /build

# crates 源与缓存：Cargo.lock 不变时不会重新下载依赖
RUN if [ -n "$CARGO_MIRROR" ]; then \
        mkdir -p /usr/local/cargo \
        && printf '[source.crates-io]\nreplace-with = "mirror"\n\n[source.mirror]\nregistry = "%s"\n' "$CARGO_MIRROR" \
           >> /usr/local/cargo/config.toml; \
    fi

# 先只编译依赖：改动业务代码时这一层能命中缓存，重建通常 1 分钟内完成
COPY backend/Cargo.toml backend/Cargo.lock ./
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/build/target \
    mkdir -p src && echo 'fn main() {}' > src/main.rs \
    && cargo build --release \
    && rm -rf src target/release/deps/redteam_atlas* target/release/redteam-atlas*

# sqlx 的 migrate! 在编译期读取 migrations 目录，必须一起进来
COPY backend/src ./src
COPY backend/migrations ./migrations

# COPY 会保留源文件的时间戳，而上一层的占位 main.rs 是构建那一刻写的，
# 时间比源码还新。Cargo 按 mtime 判断有无改动，会误认为「没变」而直接复用
# 空壳产物 fn main() {}——镜像能构建成功，但容器启动即退出、退出码 0、无日志。
# 这里先把时间戳刷成当前，并清掉本 crate 的指纹，逼 Cargo 重新编译。
RUN find src -type f -exec touch {} +

# 清掉本 crate 的指纹。target 是缓存挂载，必须在这个 RUN 里挂着才有意义
RUN --mount=type=cache,target=/build/target \
    cargo clean -p redteam-atlas || true

# 编译真实代码。target 与 registry 都是缓存挂载——注意缓存卷的内容不会写回镜像层，
# 所以必须在同一个 RUN 里把产物复制到普通目录 /build/out，后面的跨阶段 COPY 才拿得到。
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/build/target \
    cargo build --release \
    && mkdir -p /build/out \
    && cp target/release/redteam-atlas /build/out/redteam-atlas

# 构建完检查产物大小：占位程序的产物只有几百 KB，真实程序是十几 MB。
# 万一还是复用了空壳，这里直接让构建失败，不要产出一个起不来的镜像。
RUN size=$(stat -c%s /build/out/redteam-atlas) \
    && echo "二进制大小：$size 字节" \
    && [ "$size" -gt 1000000 ] \
    || (echo "构建产物只有 ${size:-0} 字节，疑似复用了占位二进制 fn main(){}" >&2; exit 1)

# ---------- 运行时 ----------
FROM debian:bookworm-slim
ARG APT_MIRROR=""

RUN if [ -n "$APT_MIRROR" ]; then \
        sed -i "s|deb.debian.org|$APT_MIRROR|g" /etc/apt/sources.list.d/debian.sources 2>/dev/null \
        || sed -i "s|deb.debian.org|$APT_MIRROR|g" /etc/apt/sources.list; \
    fi

RUN --mount=type=cache,target=/var/cache/apt,sharing=locked \
    --mount=type=cache,target=/var/lib/apt,sharing=locked \
    apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates tini curl gosu \
    && rm -rf /var/lib/apt/lists/* \
    && useradd --system --create-home --shell /usr/sbin/nologin atlas

LABEL org.opencontainers.image.title="RedTeam Atlas" \
      org.opencontainers.image.version="4.4.1"

WORKDIR /app
ENV PORT=8080 \
    HOST=0.0.0.0 \
    ATLAS_DATA_DIR=/data \
    ATLAS_DIST_DIR=/app/dist \
    RUST_LOG=info

COPY --from=api /build/out/redteam-atlas /usr/local/bin/redteam-atlas
COPY --from=web /build/dist /app/dist
COPY scripts/docker-entrypoint.sh /usr/local/bin/docker-entrypoint.sh

RUN chmod +x /usr/local/bin/docker-entrypoint.sh \
    && mkdir -p /data \
    && chown -R atlas:atlas /data /app

VOLUME ["/data"]
EXPOSE 8080

HEALTHCHECK --interval=30s --timeout=5s --start-period=10s --retries=3 \
  CMD curl -fsS http://127.0.0.1:8080/ >/dev/null || exit 1

# 入口以 root 进来，修正 /data 属主后降权到 atlas
ENTRYPOINT ["/usr/bin/tini", "--", "/usr/local/bin/docker-entrypoint.sh"]
CMD ["redteam-atlas"]
# ---------- 前端构建 ----------
FROM node:22-alpine AS web
WORKDIR /build
COPY frontend/package*.json ./
RUN npm ci --no-fund --no-audit
COPY frontend/ ./
RUN npm run build

# ---------- 后端构建 ----------
FROM rust:1-slim-bookworm AS api
RUN apt-get update \
    && apt-get install -y --no-install-recommends build-essential pkg-config \
    && rm -rf /var/lib/apt/lists/*
WORKDIR /build

# 先只编译依赖：改动业务代码时这一层能命中缓存，重建通常 1 分钟内完成
COPY backend/Cargo.toml backend/Cargo.lock ./
RUN mkdir -p src && echo 'fn main() {}' > src/main.rs \
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
RUN cargo clean -p redteam-atlas || true

# 构建完检查产物大小：占位程序的产物只有几百 KB，真实程序是十几 MB。
# 万一还是复用了空壳，这里直接让构建失败，不要产出一个起不来的镜像。
RUN cargo build --release
RUN size=$(stat -c%s target/release/redteam-atlas) \
    && echo "二进制大小：$size 字节" \
    && [ "$size" -gt 1000000 ] \
    || (echo "构建产物只有 ${size:-0} 字节，疑似复用了占位二进制 fn main(){}" >&2; exit 1)

# ---------- 运行时 ----------
FROM debian:bookworm-slim
LABEL org.opencontainers.image.title="RedTeam Atlas" \
      org.opencontainers.image.version="3.6.2"

RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates tini curl gosu \
    && rm -rf /var/lib/apt/lists/* \
    && useradd --system --create-home --shell /usr/sbin/nologin atlas

WORKDIR /app
ENV PORT=8080 \
    HOST=0.0.0.0 \
    ATLAS_DATA_DIR=/data \
    ATLAS_DIST_DIR=/app/dist \
    RUST_LOG=info

COPY --from=api /build/target/release/redteam-atlas /usr/local/bin/redteam-atlas
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
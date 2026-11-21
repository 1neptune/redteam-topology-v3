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
RUN cargo build --release

# ---------- 运行时 ----------
FROM debian:bookworm-slim
LABEL org.opencontainers.image.title="RedTeam Atlas" \
      org.opencontainers.image.version="3.4.0"

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
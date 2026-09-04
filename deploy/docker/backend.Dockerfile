# syntax=docker/dockerfile:1.7

# ---- 前端:纯 SPA 静态产物,由后端 axum 直接托管 ----
FROM node:22-bookworm-slim AS frontend
WORKDIR /src/frontend

COPY frontend/package.json frontend/package-lock.json ./
RUN npm ci --no-audit --no-fund --registry=https://registry.npmmirror.com/
COPY frontend ./

RUN npm run build

# ---- 后端:编译 API 二进制 ----
FROM rust:1.94-bookworm AS builder
WORKDIR /src

# 容器内直连 crates.io 易超时,改用 rsproxy 镜像源拉取依赖。
COPY <<'CARGOCONF' /usr/local/cargo/config.toml
[source.crates-io]
replace-with = "rsproxy-sparse"

[source.rsproxy-sparse]
registry = "sparse+https://rsproxy.cn/index/"
CARGOCONF

# 先复制依赖描述，便于 Docker 利用依赖编译缓存。
COPY Cargo.toml Cargo.lock ./
COPY backend/Cargo.toml backend/Cargo.toml
COPY backend/entity/Cargo.toml backend/entity/Cargo.toml
COPY backend/migrations/Cargo.toml backend/migrations/Cargo.toml

# 复制源码后编译后端 API(migration 已作为依赖编译进二进制,启动时自动应用)。
COPY backend backend
RUN cargo build --release -p projecty-api

# ---- 运行时:单镜像 = API + 前端静态文件 ----
FROM debian:bookworm-slim AS runtime
RUN apt-get update \
    && apt-get install --no-install-recommends -y ca-certificates \
    && rm -rf /var/lib/apt/lists/* \
    && useradd --create-home --uid 10001 projecty \
    && mkdir -p /var/lib/projecty/uploads \
    && chown -R projecty:projecty /var/lib/projecty

COPY --from=builder /src/target/release/projecty-api /usr/local/bin/projecty-api
COPY --from=frontend /src/frontend/build /usr/share/projecty/static

ENV PROJECTY_UPLOAD_DIR=/var/lib/projecty/uploads
ENV PROJECTY_STATIC_DIR=/usr/share/projecty/static

USER projecty
EXPOSE 8080
CMD ["/usr/local/bin/projecty-api"]

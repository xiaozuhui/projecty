# syntax=docker/dockerfile:1.7

FROM rust:1.86-bookworm AS builder
WORKDIR /src

# 先复制依赖描述，便于 Docker 利用依赖编译缓存。
COPY Cargo.toml Cargo.lock ./
COPY backend/Cargo.toml backend/Cargo.toml
COPY backend/entity/Cargo.toml backend/entity/Cargo.toml
COPY backend/migrations/Cargo.toml backend/migrations/Cargo.toml

# 复制源码后编译后端 API 和数据库迁移命令。
COPY backend backend
COPY backend/entity backend/entity
COPY backend/migrations backend/migrations
RUN cargo build --release -p projecty-api -p projecty-migration

FROM debian:bookworm-slim AS runtime
RUN apt-get update \
    && apt-get install --no-install-recommends -y ca-certificates \
    && rm -rf /var/lib/apt/lists/* \
    && useradd --create-home --uid 10001 projecty

COPY --from=builder /src/target/release/projecty-api /usr/local/bin/projecty-api
COPY --from=builder /src/target/release/projecty-migration /usr/local/bin/projecty-migration

USER projecty
EXPOSE 8080
CMD ["/usr/local/bin/projecty-api"]

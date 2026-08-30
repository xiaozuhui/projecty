# syntax=docker/dockerfile:1.7

FROM rust:1.86-bookworm AS builder
WORKDIR /src

# 先复制依赖描述，便于 Docker 利用依赖编译缓存。
COPY Cargo.toml Cargo.lock ./
COPY apps/api/Cargo.toml apps/api/Cargo.toml
COPY entity/Cargo.toml entity/Cargo.toml
COPY migration/Cargo.toml migration/Cargo.toml

# 复制源码后编译后端 API 和数据库迁移命令。
COPY apps/api apps/api
COPY entity entity
COPY migration migration
RUN cargo build --release -p projecty-api -p migration

FROM debian:bookworm-slim AS runtime
RUN apt-get update \
    && apt-get install --no-install-recommends -y ca-certificates \
    && rm -rf /var/lib/apt/lists/* \
    && useradd --create-home --uid 10001 projecty

COPY --from=builder /src/target/release/projecty-api /usr/local/bin/projecty-api
COPY --from=builder /src/target/release/migration /usr/local/bin/projecty-migration

USER projecty
EXPOSE 8080
CMD ["/usr/local/bin/projecty-api"]

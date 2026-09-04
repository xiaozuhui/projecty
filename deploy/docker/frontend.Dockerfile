# syntax=docker/dockerfile:1.7

FROM node:22-bookworm-slim AS builder
WORKDIR /src/frontend

COPY frontend/package.json ./
RUN npm install --no-audit --no-fund --registry=https://registry.npmmirror.com/
COPY frontend ./

RUN npm run build

FROM node:22-bookworm-slim AS runtime
WORKDIR /app
ENV NODE_ENV=production
ENV HOST=0.0.0.0
ENV PORT=3000
# adapter-node 请求体上限只认这个环境变量(默认 512KB 会挡住附件上传),
# 只能镜像内写死:50MiB + 64KiB 表单余量,与后端常量 UPLOAD_MAX_BYTES 对齐。
# (不能在 hooks.server.ts 里设——应用模块是动态加载的,晚于 handler 读取该值。)
ENV BODY_SIZE_LIMIT=52494336

COPY --from=builder /src/frontend/build ./build
COPY --from=builder /src/frontend/node_modules ./node_modules

EXPOSE 3000
CMD ["node", "build"]

# syntax=docker/dockerfile:1.7

FROM node:22-bookworm-slim AS builder
WORKDIR /src/frontend

COPY frontend/package.json ./
RUN npm install --no-audit --no-fund
COPY frontend ./

RUN npm run build

FROM node:22-bookworm-slim AS runtime
WORKDIR /app
ENV NODE_ENV=production
ENV HOST=0.0.0.0
ENV PORT=3000

COPY --from=builder /src/frontend/build ./build
COPY --from=builder /src/frontend/node_modules ./node_modules

EXPOSE 3000
CMD ["node", "build"]

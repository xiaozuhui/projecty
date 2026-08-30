# syntax=docker/dockerfile:1.7

FROM node:22-bookworm-slim AS builder
WORKDIR /src/apps/web

COPY apps/web/package.json ./
RUN npm install --no-audit --no-fund
COPY apps/web ./

ARG PUBLIC_API_BASE_URL=/api/v1
ENV PUBLIC_API_BASE_URL=${PUBLIC_API_BASE_URL}
RUN npm run build

FROM node:22-bookworm-slim AS runtime
WORKDIR /app
ENV NODE_ENV=production
ENV HOST=0.0.0.0
ENV PORT=3000

COPY --from=builder /src/apps/web/build ./build
COPY --from=builder /src/apps/web/node_modules ./node_modules

EXPOSE 3000
CMD ["node", "build"]

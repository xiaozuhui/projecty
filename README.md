# Projecty

Projecty 是一个单公司内部使用的项目管理服务骨架，目标是提供类似 JIRA 的项目、任务、子任务、部门授权和操作日志能力，但第一阶段只聚焦项目管理。

## 技术栈

- 后端：Rust + Axum + SeaORM + PostgreSQL
- 前端：SvelteKit + TypeScript + 原生 CSS
- 鉴权：本地账号密码 + JWT

## 目录

```text
backend                    Rust Axum 后端应用
backend/src/domain         领域类型与不变式
backend/src/application    应用服务与权限策略
backend/src/infrastructure SeaORM 数据库连接等基础设施
backend/src/modules        按业务领域组织的 HTTP 模块
backend/entity             SeaORM Entity 定义
backend/migrations          SeaORM migrations 包
frontend                   SvelteKit Web 前端
frontend/src/routes        多页面路由
frontend/src/lib           前端组件、API 客户端和样式
deploy/docker              Docker 构建与组合部署配置
docs                       领域拆分设计文档
```

## 本地开发命令

```bash
cargo fmt --all
cargo run -p projecty-api
cd frontend && npm install && npm run dev
```

> 本仓库不内置 Tailwind CSS，不使用外部字体 CDN；Web 样式参考设计稿中的浅色工作台风格。Vite 开发服务器会将 `/api` 和 `/healthz` 代理到本地 Axum 服务。

## Docker 部署结构

前端和后端保持两个独立应用：`frontend` 只负责 SvelteKit，`backend` 只负责 Axum。Docker 构建阶段分别编译两者，部署时由 Nginx 组合为同一个访问入口，PostgreSQL 单独运行：

```text
deploy/docker/
├── backend.Dockerfile   Rust API 与 SeaORM migrations 构建
├── frontend.Dockerfile  SvelteKit Web 构建
├── nginx.conf           / 与 /api/ 路由
└── compose.yml          Backend、Frontend、Nginx、PostgreSQL 组合部署
```

不要在 API 镜像构建前端源码，也不要把业务领域拆成独立 Rust crate；后端业务边界按 `backend/src/{domain,application,infrastructure,modules}` 组织。

复制 `.env.example` 为 `.env`，替换数据库账号、密码和 JWT 密钥后，由部署环境执行：

```bash
docker compose -f deploy/docker/compose.yml up -d --build
```

本项目仅提供 Docker 构建配置静态骨架，本次未执行实际 Docker build。

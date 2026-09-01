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

前端和后端保持两个独立应用：`frontend` 只负责 SvelteKit，`backend` 只负责 Axum。Docker 构建阶段分别编译两者，运行时通过独立端口提供服务，PostgreSQL 单独运行：

```text
deploy/docker/
├── backend.Dockerfile   Rust API 与 SeaORM migrations 构建
├── frontend.Dockerfile  SvelteKit Web 构建
└── compose.yml          Backend、Frontend、PostgreSQL 组合部署
```

不要在 API 镜像构建前端源码，也不要把业务领域拆成独立 Rust crate；后端业务边界按 `backend/src/{domain,application,infrastructure,modules}` 组织。

复制 `.env.example` 为 `.env`，替换数据库账号、密码、JWT 密钥与宿主机端口（`PROJECTY_BACKEND_PORT` / `PROJECTY_FRONTEND_PORT`）后，由部署环境执行：

```bash
docker compose -f deploy/docker/compose.yml up -d --build
```

默认访问地址（端口取自 `.env`，以下为默认值）：

- 前端：`http://127.0.0.1:28080`（浏览器只需访问前端这一个源）
- 后端：`http://127.0.0.1:28081`
- 健康检查：`http://127.0.0.1:28081/healthz`

浏览器侧 API 走同源相对路径 `/api/v1`，由前端容器内 `frontend/src/hooks.server.ts` 反向代理转发到 Docker 网络内的后端容器（`INTERNAL_API_ORIGIN`，默认 `http://backend:8080`，容器名即 docker 网络内的地址）。因此后端宿主机端口即使变化，浏览器也无需感知；生产环境需要统一域名、HTTPS 时只需把网关指到前端端口。

### 初始化超级管理员

部署启动后（backend 容器处于运行状态），在后端容器内执行内置的运维子命令创建第一个超级管理员：

```bash
docker compose -f deploy/docker/compose.yml exec backend \
  projecty-api admin create \
  --account admin \
  --password '替换为强密码' \
  --display-name '超级管理员'
```

- 容器内已注入 `DATABASE_URL`，不需要再传 `--database-url`；本地裸跑二进制时才需要通过该参数或环境变量指定数据库连接串。
- 约束：账号 2-64 个字符，密码 8-128 个字符；账号已存在时命令直接报错退出，不会覆盖已有账号或密码。
- 命令执行前会自动应用数据库迁移，全新数据库也可以先建管理员再启动服务。
- 密码会留在 shell 历史里，初始化完建议清理或尽快在前端「个人设置」修改。
- 其余普通用户由超级管理员在前端「用户」页面单个创建或批量导入，不需要命令行。

本项目仅提供 Docker 构建配置静态骨架，本次未执行实际 Docker build。

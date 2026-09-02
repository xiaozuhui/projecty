# Projecty

Projecty 是一个面向单公司内部的项目管理系统，提供类 JIRA 的项目协作能力：项目与任务管理、部门组织、成员授权和操作审计。

## 功能概览

- **项目**：项目空间、状态、里程碑、成员管理（直接添加或按部门授权）
- **任务**：子任务、任务编号、负责人与评审人、状态流转权限、评论、附件
- **组织**：部门树、用户多部门归属、按部门的项目可见性
- **用户**：账号密码登录、超级管理员/普通用户两种系统角色、Excel 批量导入
- **个人设置**：修改姓名、邮箱与密码
- **审计**：关键操作记录操作日志，支持全局搜索

## 技术栈

- 后端：Rust + Axum + SeaORM + PostgreSQL（JWT 鉴权、Argon2 密码哈希）
- 前端：SvelteKit + Svelte 5 + TypeScript + 原生 CSS
- 部署：Docker Compose（前端、后端、PostgreSQL 三容器）

## 目录结构

```text
backend/        Rust 后端（src/modules 按业务领域组织，migrations 数据库迁移）
frontend/       SvelteKit 前端
deploy/docker/  Docker 构建与组合部署配置
docs/           领域与设计文档
```

## 本地开发

后端默认监听 `8080`，启动时自动应用数据库迁移；前端开发服务器会把 `/api` 代理到本地后端。

```bash
cargo run -p projecty-api        # 需配置 DATABASE_URL、JWT_SECRET、JWT_ISSUER
cd frontend && npm install && npm run dev
```

## 部署

前端与后端是两个独立应用，PostgreSQL 单独运行，由 Docker Compose 组合部署。浏览器只访问前端一个源，API 走同源相对路径 `/api/v1`，由前端容器内的 `hooks.server.ts` 反向代理转发到 Docker 网络内的后端容器，因此后端宿主机端口变化不影响浏览器侧。

```bash
# 1. 准备配置：复制 deploy/docker/.env.example 为 .env，
#    替换数据库账号密码、JWT 密钥与宿主机端口
# 2. 构建并启动
docker compose -f deploy/docker/compose.yml up -d --build
```

默认地址：前端 `http://127.0.0.1:28080`，后端健康检查 `http://127.0.0.1:28081/healthz`。生产环境统一域名、HTTPS 时把网关指到前端端口即可。

### 初始化超级管理员

服务启动后，在后端容器内执行一次：

```bash
docker compose -f deploy/docker/compose.yml exec backend \
  projecty-api admin create --account admin --password '<密码>' --display-name '超级管理员'
```

其余普通用户由超级管理员在前端「用户管理」页面创建或批量导入，无需命令行。

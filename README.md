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
- 部署：Docker Compose（后端+前端单镜像、PostgreSQL 两容器）

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

单镜像部署：前端构建为纯静态产物打进后端镜像，由 axum 直接托管（无 Node 运行时）；PostgreSQL 单独容器，Docker Compose 组合。浏览器只访问后端一个源，API 走同源相对路径 `/api/v1`。

镜像在本地构建，服务器只加载、不编译。常规流程（服务器为 x86 时构建用 `DOCKER_PLATFORM=linux/amd64 ./build.sh`）：

```bash
# 1. 本地构建镜像
./build.sh
# 2. 流式导出上传(本地与服务器都不落盘)
docker save projecty-backend:latest | gzip | ssh <服务器> 'gzip -dc | docker load'
# 3. 服务器拉起(首次先复制 deploy/docker/.env.example 为 .env,填数据库与 JWT 配置)
ssh <服务器> 'cd ~/projecty && docker compose -f deploy/docker/compose.yml up -d'
```

默认地址：页面 `http://<服务器IP>:28080`，健康检查 `http://<服务器IP>:28080/healthz`。

### 初始化超级管理员

服务启动后，在后端容器内执行一次：

```bash
docker compose -f deploy/docker/compose.yml exec backend \
  projecty-api admin create --account admin --password '<密码>' --display-name '超级管理员'
```

其余普通用户由超级管理员在前端「用户管理」页面创建或批量导入，无需命令行。

# Projecty

Projecty 是一个单公司内部使用的项目管理服务骨架，目标是提供类似 JIRA 的项目、任务、子任务、部门授权和操作日志能力，但第一阶段只聚焦项目管理。

## 技术栈

- 后端：Rust + Axum + SeaORM + PostgreSQL
- 前端：SvelteKit + TypeScript + 原生 CSS
- 鉴权：本地账号密码 + JWT

## 目录

```text
apps/api                  Rust Axum API 服务
apps/web                  SvelteKit Web 端
crates/domain             领域类型与不变式
crates/application        应用服务与权限策略
crates/infrastructure     SeaORM 数据库连接等基础设施
entity                    SeaORM 实体定义入口
migration                 SeaORM Migration
docs                      领域拆分设计文档
```

## 本地开发命令

```bash
cargo fmt --all
cargo run -p projecty-api
cd apps/web && npm install && npm run dev
```

> 本仓库不内置 Tailwind CSS，不使用外部字体 CDN；Web 样式参考设计稿中的浅色工作台风格。

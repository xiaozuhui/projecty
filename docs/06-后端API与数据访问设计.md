# 06 - 后端 API 与数据访问设计

> **文档状态**：Draft 0.4  
> **上级文档**：[00-项目管理服务设计文档索引](./00-项目管理服务设计文档.md)

---

## 1. 推荐工程结构

```text
projecty/
├── apps/
│   ├── api/
│   │   └── src/
│   │       ├── main.rs
│   │       ├── app.rs
│   │       ├── config.rs
│   │       ├── state.rs
│   │       ├── http/
│   │       │   ├── routes.rs
│   │       │   ├── middleware.rs
│   │       │   ├── extractors.rs
│   │       │   └── error.rs
│   │       └── modules/
│   │           ├── auth/
│   │           ├── departments/
│   │           ├── projects/
│   │           ├── tasks/
│   │           ├── milestones/
│   │           ├── comments/
│   │           └── audit/
│   └── web/
│       ├── src/routes/
│       ├── src/lib/
│       └── svelte.config.js
├── crates/
│   ├── domain/
│   ├── application/
│   └── infrastructure/
├── migration/
├── entity/
├── docs/
└── Cargo.toml
```

不要让 Axum handler 直接拼接复杂查询、修改多个表并决定权限。handler 只做 HTTP 适配，业务放 application service。

---

## 2. Axum 请求链路

```text
HTTP request
  -> tracing/request-id middleware
  -> jwt/auth middleware
  -> route handler
  -> application use case
  -> authorization policy
  -> repository / SeaORM
  -> PostgreSQL
  -> response DTO
```

`AppState` 示例：

```rust
#[derive(Clone)]
pub struct AppState {
    pub db: DatabaseConnection,
    pub config: Arc<AppConfig>,
    pub jwt_service: Arc<JwtService>,
    pub authorization: Arc<AuthorizationService>,
}
```

---

## 3. SeaORM 使用原则

- migration 是数据库 schema 的权威变更记录。
- entity 与 migration 同步。
- 复杂查询封装在 repository/query object 中。
- 更新任务、状态、编号、逻辑删除、操作日志时明确事务边界。
- 批量列表必须有明确排序和分页。
- 默认禁止无上限 `.all()` 读取大表。
- 返回 API DTO，不直接把 SeaORM model 当公开接口契约。

---

## 4. API 响应格式

成功：

```json
{
  "data": {},
  "meta": {
    "request_id": "req_..."
  }
}
```

错误：

```json
{
  "error": {
    "code": "TASK_VERSION_CONFLICT",
    "message": "任务已被其他用户更新",
    "fields": {},
    "request_id": "req_..."
  }
}
```

---

## 5. 核心 API

```text
POST   /api/v1/auth/login
POST   /api/v1/auth/refresh
POST   /api/v1/auth/logout
GET    /api/v1/me
PATCH  /api/v1/me/password

GET    /api/v1/departments
POST   /api/v1/departments
PATCH  /api/v1/departments/{department_id}
POST   /api/v1/departments/{department_id}/delete
GET    /api/v1/departments/{department_id}/projects

GET    /api/v1/projects
POST   /api/v1/projects
GET    /api/v1/projects/{project_key}
PATCH  /api/v1/projects/{project_key}
POST   /api/v1/projects/{project_key}/archive
POST   /api/v1/projects/{project_key}/restore
POST   /api/v1/projects/{project_key}/delete

GET    /api/v1/projects/{project_key}/members
POST   /api/v1/projects/{project_key}/members
PATCH  /api/v1/projects/{project_key}/members/{user_id}
POST   /api/v1/projects/{project_key}/members/{user_id}/revoke
GET    /api/v1/projects/{project_key}/department-grants
POST   /api/v1/projects/{project_key}/department-grants
POST   /api/v1/projects/{project_key}/department-grants/{department_id}/revoke

GET    /api/v1/projects/{project_key}/tasks
POST   /api/v1/projects/{project_key}/tasks
GET    /api/v1/tasks/{task_key}
PATCH  /api/v1/tasks/{task_key}
POST   /api/v1/tasks/{task_key}/transition
POST   /api/v1/tasks/{task_key}/delete
POST   /api/v1/tasks/{task_key}/restore
GET    /api/v1/tasks/{task_key}/subtasks
POST   /api/v1/tasks/{task_key}/subtasks

GET    /api/v1/projects/{project_key}/statuses
PATCH  /api/v1/projects/{project_key}/statuses/order
GET    /api/v1/projects/{project_key}/milestones
POST   /api/v1/projects/{project_key}/milestones
PATCH  /api/v1/milestones/{id}
POST   /api/v1/milestones/{id}/delete

GET    /api/v1/tasks/{task_key}/comments
POST   /api/v1/tasks/{task_key}/comments
POST   /api/v1/comments/{id}/delete

GET    /api/v1/projects/{project_key}/logs
GET    /api/v1/projects/{project_key}/logs/export
GET    /api/v1/tasks/{task_key}/logs
GET    /api/v1/tasks/{task_key}/logs/export
GET    /api/v1/admin/operation-logs/export

GET    /api/v1/search?q=...
```

说明：

- 所有 `delete/revoke/archive` 接口都是逻辑删除或状态变更，不物理删除。
- 资源解析先锁定项目，再计算当前用户对该项目的有效角色。
- 禁止“先按 task key 全局查到记录并返回内容，再补做权限判断”。

---

## 6. 分页与索引

列表接口统一支持：

```text
page_size
cursor
sort
filter
```

高频索引建议：

```text
department_closure (descendant_id, ancestor_id)
user_departments (department_id, user_id) where revoked_at is null
projects (owner_department_id, status, updated_at desc) where deleted_at is null
project_members (user_id, project_id, role) where revoked_at is null
project_department_grants (department_id, project_id, role) where revoked_at is null
tasks (project_id, deleted_at, status_id, position)
tasks (project_id, parent_task_id, position) where deleted_at is null
tasks (project_id, assignee_id, due_date) where deleted_at is null
tasks (project_id, number)
tasks (project_id, updated_at desc) where deleted_at is null
milestones (project_id, due_date) where deleted_at is null
operation_logs (project_id, created_at desc)
operation_logs (task_id, created_at desc) where task_id is not null
```

索引需要通过真实查询计划校验，不为了“看起来完整”给每个字段都建索引。

---

## 7. 百万任务处理策略

1. 看板按项目和状态列分页，例如每列先取 50 条。
2. 列表使用 cursor/keyset pagination，避免深分页 `offset 900000`。
3. 默认查询只返回 `deleted_at is null` 的任务。
4. 已删除任务进入单独回收站视图。
5. “我的任务”使用 `assignee_id + due_date/updated_at` 索引。
6. 项目概览统计可维护 counter 表或异步刷新。
7. 每个高频列表接口必须有固定排序，例如 `(updated_at desc, id desc)` 或 `(position, id)`。
8. 阶段 0 准备 1,000,000 条任务种子数据，验证项目列表、看板首屏、任务列表、任务详情、日志查询和日志导出的实际耗时。

# 07 - 前端 SvelteKit 与视觉设计

> **文档状态**：Draft 0.4  
> **上级文档**：[00-项目管理服务设计文档索引](./00-项目管理服务设计文档.md)

---

## 1. 前端形态

Projecty 不是单页 Demo，而是复杂多页面 Web 应用。推荐使用 SvelteKit 文件路由、嵌套路由布局、页面级数据加载和表单提交。

---

## 2. 路由树

```text
src/routes/
├── (auth)/
│   ├── login/+page.svelte
│   └── forgot-password/+page.svelte
├── (app)/
│   ├── +layout.server.ts                 # JWT 登录态、当前用户、导航数据
│   ├── +layout.svelte                    # AppShell：侧栏、顶部栏、通知入口
│   ├── +page.svelte                      # 首页：我的任务、参与项目、近期日志
│   ├── departments/
│   │   ├── +page.svelte                  # 部门目录；父部门可见下级部门
│   │   └── [departmentId]/+page.svelte   # 部门项目和成员概览
│   ├── projects/
│   │   ├── +page.svelte                  # 项目目录，可按部门/负责人筛选
│   │   ├── new/+page.svelte              # 新建项目
│   │   └── [projectKey]/
│   │       ├── +layout.svelte            # 项目上下文导航与项目工具栏
│   │       ├── +page.svelte              # 项目概览
│   │       ├── board/+page.svelte        # 看板
│   │       ├── list/+page.svelte         # 列表
│   │       ├── timeline/+page.svelte     # 时间线/甘特化轻量视图
│   │       ├── calendar/+page.svelte     # 日历
│   │       ├── subtasks/+page.svelte     # 父任务-子任务分解视图
│   │       ├── milestones/+page.svelte
│   │       ├── members/+page.svelte      # 直接成员 + 部门授权
│   │       ├── logs/+page.svelte         # 项目操作日志
│   │       └── settings/+page.svelte
│   ├── tasks/[taskKey]/+page.svelte      # 任务独立详情页
│   ├── search/+page.svelte
│   ├── notifications/+page.svelte        # 阶段 2 站内通知
│   └── settings/
│       ├── profile/+page.svelte
│       ├── account/+page.svelte
│       └── system/+page.svelte           # 超级管理员可见
└── api/                                  # 仅在需要 SvelteKit BFF 时使用
```

---

## 3. 组件边界

```text
src/lib/
├── components/
│   ├── app-shell/
│   ├── navigation/
│   ├── project/
│   ├── task/
│   ├── milestone/
│   ├── operation-log/
│   └── ui/
├── features/
│   ├── board/
│   ├── task-list/
│   ├── timeline/
│   ├── calendar/
│   └── project-settings/
├── api/
├── stores/
├── schemas/
└── styles/
    ├── tokens.css
    ├── globals.css
    └── components.css
```

跨页面 store 只保存当前用户、当前部门筛选、导航折叠、主题和通知未读数。项目任务数据由页面 load/API cache 管理，避免把所有后端数据塞入一个全局 store。

---

## 4. 视觉方向

参考用户 HTML 中的浅色项目工作台风格：

- 白色侧栏。
- 雾灰主背景。
- 蓝色主色。
- 卡片化任务。
- 顶部操作栏。
- 任务状态、负责人、优先级可快速扫描。

不使用 Tailwind CSS，不继续堆叠过多渐变和装饰，核心记忆点放在：

```text
清晰的项目导航脊柱 + 高对比任务状态色带 + 可快速扫描的信息层级
```

---

## 5. 字体

不使用 Inter、Intel One、Roboto 作为主字体，也不依赖外部字体 CDN。

推荐：

```css
:root {
  --font-ui: "Noto Sans SC", "PingFang SC", "Hiragino Sans GB",
             "Microsoft YaHei", sans-serif;
  --font-display: "Noto Sans SC", "PingFang SC", sans-serif;
  --font-mono: "SFMono-Regular", "Cascadia Code", "JetBrains Mono",
               monospace;
}
```

如需保证 Linux 中文一致性，可以自托管经过许可的 `Noto Sans SC` 子集。

---

## 6. 色彩 token

```css
:root {
  --color-primary: #4f7df3;
  --color-primary-strong: #365fd6;
  --color-primary-soft: #eef2ff;
  --color-bg: #f4f6f9;
  --color-surface: #ffffff;
  --color-border: #e6eaf0;
  --color-border-strong: #cfd6e4;
  --color-text: #1a1a2e;
  --color-text-secondary: #3d465e;
  --color-text-muted: #8e98b0;
  --color-success: #2fa36b;
  --color-warning: #d68b2d;
  --color-danger: #d85462;
  --color-info: #5a78d6;

  --shadow-sm: 0 2px 8px rgba(26, 37, 67, .05);
  --shadow-md: 0 12px 32px rgba(26, 37, 67, .10);
  --radius-sm: 8px;
  --radius-md: 12px;
  --radius-lg: 16px;
}
```

状态颜色必须同时有文字、图标或形状表达，不能只依赖颜色。

---

## 7. 关键页面

### 看板页

- 列头包含状态名称、任务数、列菜单。
- 每列分批加载，避免百万任务量下拉爆接口。
- 拖拽失败恢复原位置并显示错误原因。
- 桌面端看板容器横向滚动。
- 小屏不把列压到 160px 以下。

### 列表页

- 筛选条件同步到 URL。
- 表格列：编号、标题、状态、优先级、负责人、里程碑、截止日期、更新时间。
- 第一版批量操作只开放低风险能力：批量改状态、批量改负责人、批量加标签。
- 删除不作为批量默认入口。

### 任务详情页/抽屉

1. 标题和任务编号。
2. 状态、优先级、负责人、截止日期、里程碑。
3. 描述。
4. 父任务/子任务区，只展示两层。
5. 阻塞关系与依赖关系。
6. 评论与操作日志时间线。
7. 危险操作区：删除、恢复，按权限显示。

---

## 8. 响应式断点

| 断点 | 布局行为 |
|---|---|
| `>= 1280px` | 完整侧栏、双栏详情、看板多列、列表显示完整字段 |
| `1024–1279px` | 侧栏可折叠，详情改为抽屉，列表减少次要列 |
| `768–1023px` | 侧栏默认折叠，看板保持横向滚动，工具条允许换行 |
| `< 768px` | 侧栏抽屉、顶部操作收进菜单、详情全屏、列表改为卡片化行 |
| `< 480px` | 新建/编辑表单单列，按钮使用底部固定操作栏 |

必须在 1280×800、1024×768、768×1024、390×844 四类 viewport 实际检查，不能只以 CSS 编译成功作为视觉验收。

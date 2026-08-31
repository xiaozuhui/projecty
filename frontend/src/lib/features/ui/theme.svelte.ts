// 主题状态:dataset.theme 驱动 CSS 令牌切换,localStorage 持久化(键名与 session 一致用 projecty.* 前缀)。
const THEME_KEY = 'projecty.theme';

export type Theme = 'dark' | 'light';

export const theme = $state<{ current: Theme }>({ current: 'dark' });

/** 从 documentElement 同步初始主题(app.html 防闪烁脚本已经先设好)。 */
export function initTheme() {
  theme.current = document.documentElement.dataset.theme === 'light' ? 'light' : 'dark';
}

export function toggleTheme() {
  const next: Theme = theme.current === 'dark' ? 'light' : 'dark';
  theme.current = next;
  document.documentElement.dataset.theme = next;
  try {
    localStorage.setItem(THEME_KEY, next);
  } catch {
    // 隐私模式等场景下写入失败,仅保留本次会话的主题切换。
  }
}

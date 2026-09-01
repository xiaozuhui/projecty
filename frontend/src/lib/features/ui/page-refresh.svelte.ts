import { page } from '$app/state';

/**
 * 页面数据随导航重载:
 * $effect 依赖路由路径,挂载时执行一次,同路由参数变化(SvelteKit 不重挂组件)时重跑;
 * pageshow.persisted 兜底浏览器往返缓存恢复的旧快照。
 * 不用 afterNavigate:它在整页首次加载/刷新时不会触发,页面会永远停在加载态。
 */
export function bindReload(reload: () => void) {
  $effect(() => {
    void page.url.pathname;
    void page.url.search;
    reload();
  });
  $effect(() => {
    const onPageshow = (event: PageTransitionEvent) => {
      if (event.persisted) reload();
    };
    window.addEventListener('pageshow', onPageshow);
    return () => window.removeEventListener('pageshow', onPageshow);
  });
}

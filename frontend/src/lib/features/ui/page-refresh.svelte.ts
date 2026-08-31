import { afterNavigate } from '$app/navigation';

/**
 * 页面数据随导航重载:afterNavigate 覆盖首次进入与同路由参数变化
 * (SvelteKit 不会因参数变化重挂组件,onMount 不会重跑);
 * pageshow.persisted 兜底浏览器往返缓存恢复的旧快照。
 */
export function bindReload(reload: () => void) {
  afterNavigate(() => {
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

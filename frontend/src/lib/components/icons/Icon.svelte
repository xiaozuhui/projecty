<script lang="ts">
  // 内联 SVG 图标:24 网格 stroke 线条,currentColor 跟随文字色,保持零运行时依赖。
  export type IconName =
    | 'home' | 'tasks' | 'projects' | 'departments' | 'search' | 'users'
    | 'bell' | 'settings' | 'sun' | 'moon' | 'plus' | 'logout' | 'image' | 'close';

  let { name, size = 16 }: { name: IconName; size?: number } = $props();

  const paths: Record<IconName, string> = {
    home: 'M3 10.5 12 3l9 7.5M5 9.5V21h5v-6h4v6h5V9.5',
    tasks: 'M5 4h14a1 1 0 0 1 1 1v14a1 1 0 0 1-1 1H5a1 1 0 0 1-1-1V5a1 1 0 0 1 1-1ZM8.5 12.5l2.5 2.5 4.5-4.5',
    projects: 'M4 7h16a1 1 0 0 1 1 1v10a1 1 0 0 1-1 1H4a1 1 0 0 1-1-1V8a1 1 0 0 1 1-1ZM9 7V5a2 2 0 0 1 2-2h2a2 2 0 0 1 2 2v2M3 12h18',
    departments: 'M9.5 3h5v4h-5zM3.5 17h5v4h-5zM15.5 17h5v4h-5zM12 7v3m0 0H6.5v7M12 10h5.5v7',
    search: 'M11 4a7 7 0 1 1 0 14 7 7 0 0 1 0-14ZM16.5 16.5 21 21',
    users: 'M9 4.5a3.5 3.5 0 1 1 0 7 3.5 3.5 0 0 1 0-7ZM3.5 20c0-3 2.5-5 5.5-5s5.5 2 5.5 5M16 5a3 3 0 0 1 0 6M17.5 15c2 .8 3 2.5 3 5',
    bell: 'M6 9a6 6 0 1 1 12 0c0 4 1.5 5.5 1.5 5.5h-15S6 13 6 9M10 19a2 2 0 0 0 4 0',
    settings: 'M4 8h7M16 8h4M14 6v4M4 16h4M13 16h7M9 14v4',
    sun: 'M12 8a4 4 0 1 1 0 8 4 4 0 0 1 0-8ZM12 2.5v2M12 19.5v2M2.5 12h2M19.5 12h2M5.2 5.2l1.4 1.4M17.4 17.4l1.4 1.4M18.8 5.2l-1.4 1.4M6.6 17.4l-1.4 1.4',
    moon: 'M20 14.5A8.5 8.5 0 0 1 9.5 4 8.5 8.5 0 1 0 20 14.5Z',
    plus: 'M12 5v14M5 12h14',
    logout: 'M9 4H6a2 2 0 0 0-2 2v12a2 2 0 0 0 2 2h3M16 8l4 4-4 4M20 12H9',
    image: 'M5 4h14a1 1 0 0 1 1 1v14a1 1 0 0 1-1 1H5a1 1 0 0 1-1-1V5a1 1 0 0 1 1-1ZM9.5 9a1.5 1.5 0 1 1 0 .01M5 17l4.5-4.5 3 3 3-3L19 17',
    close: 'M6 6l12 12M18 6 6 18',
  };
</script>

<svg width={size} height={size} viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
  <path d={paths[name]} />
</svg>

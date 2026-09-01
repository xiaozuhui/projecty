import type { MeResponse } from '$lib/api/types';

/**
 * 当前登录人共享 store:(app) 布局加载 me() 后回填,
 * 页面据此做超管专属 UI 的显隐(服务端仍是最终裁决)。
 */
class MeStore {
  current = $state<MeResponse | null>(null);

  set(user: MeResponse | null) {
    this.current = user;
  }

  get isAdmin() {
    return this.current?.system_role === 'super_admin';
  }
}

export const meStore = new MeStore();

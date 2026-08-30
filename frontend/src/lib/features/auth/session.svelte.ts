import { browser } from '$app/environment';

export type AuthSession = {
  access_token: string;
  refresh_token: string;
  token_type: 'Bearer';
  expires_in: number;
};

class SessionStore {
  accessToken = $state<string | null>(null);
  refreshToken = $state<string | null>(null);

  constructor() {
    if (browser) {
      this.accessToken = sessionStorage.getItem('projecty.access_token');
      this.refreshToken = sessionStorage.getItem('projecty.refresh_token');
    }
  }

  set(session: AuthSession) {
    this.accessToken = session.access_token;
    this.refreshToken = session.refresh_token;
    if (browser) {
      sessionStorage.setItem('projecty.access_token', session.access_token);
      sessionStorage.setItem('projecty.refresh_token', session.refresh_token);
    }
  }

  clear() {
    this.accessToken = null;
    this.refreshToken = null;
    if (browser) {
      sessionStorage.removeItem('projecty.access_token');
      sessionStorage.removeItem('projecty.refresh_token');
    }
  }
}

export const session = new SessionStore();

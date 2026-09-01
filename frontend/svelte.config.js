import adapter from '@sveltejs/adapter-node';

const config = {
  kit: {
    adapter: adapter({ out: 'build' }),
    // /api 由 hooks.server.ts 同源代理转发到后端容器;应用是纯 SPA、
    // Bearer 头鉴权且无 cookie,不存在 CSRF 面,而默认的 origin 校验会
    // 拒掉无 Origin 头的合法客户端(curl/脚本)的 form-data 上传。
    csrf: { checkOrigin: false }
  }
};

export default config;

import adapter from '@sveltejs/adapter-static';

const config = {
  kit: {
    // 纯 SPA:静态产物由后端 axum 直接托管(ServeDir + index.html 兜底),
    // 无 node 服务端,也就没有 CSRF 面(Bearer 头鉴权、无 cookie)。
    adapter: adapter({ fallback: 'index.html' })
  }
};

export default config;

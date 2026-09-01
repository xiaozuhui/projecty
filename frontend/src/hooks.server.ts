// 同源 API 代理:浏览器只访问前端源(/api/v1 相对路径),
// 请求在这里转发到 docker 网络内的后端容器(容器名:8080),
// 浏览器无需感知后端宿主机端口,也不产生跨域。
import { env } from '$env/dynamic/private';
import type { Handle } from '@sveltejs/kit';

const upstream = () => (env.INTERNAL_API_ORIGIN || 'http://127.0.0.1:8080').replace(/\/$/, '');

// 逐跳头不应跨代理转发;content-length 在重组 body 后由运行时自动重算。
const HOP_HEADERS = new Set(['host', 'connection', 'content-length', 'keep-alive', 'transfer-encoding', 'upgrade']);

function forwardable(headers: Headers) {
  return [...headers].filter(([name]) => !HOP_HEADERS.has(name.toLowerCase()));
}

export const handle: Handle = async ({ event, resolve }) => {
  if (!event.url.pathname.startsWith('/api/')) return resolve(event);

  const init: RequestInit = {
    method: event.request.method,
    headers: forwardable(event.request.headers)
  };
  // body 走缓冲而非流转发:附件上传 ≤10MB,可整体读入,绕开 undici 流式 body 的 duplex 约束。
  if (!['GET', 'HEAD'].includes(event.request.method)) {
    init.body = await event.request.arrayBuffer();
  }

  try {
    const response = await fetch(upstream() + event.url.pathname + event.url.search, init);
    return new Response(response.body, {
      status: response.status,
      statusText: response.statusText,
      headers: forwardable(response.headers)
    });
  } catch {
    return new Response(JSON.stringify({ error: { code: 'bad_gateway', message: '后端服务不可达' } }), {
      status: 502,
      headers: { 'content-type': 'application/json' }
    });
  }
};

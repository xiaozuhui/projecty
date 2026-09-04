// 同源 API 代理:浏览器只访问前端源(/api/v1 相对路径),
// 请求在这里转发到 docker 网络内的后端容器(容器名:8080),
// 浏览器无需感知后端宿主机端口,也不产生跨域。
// 转发记录一行日志(method path -> status 耗时),4xx/5xx 走 warn/error,
// 前端容器的 docker logs 由此能看到所有 API 流量。
import { env } from '$env/dynamic/private';
import type { Handle } from '@sveltejs/kit';

const upstream = () => (env.INTERNAL_API_ORIGIN || 'http://127.0.0.1:8080').replace(/\/$/, '');

// 逐跳头不应跨代理转发;content-length 在重组 body 后由运行时自动重算。
// expect(100-continue)在 body 已整体缓冲后语义失效,undici 带着它转发会挂死大请求体。
const HOP_HEADERS = new Set(['host', 'connection', 'content-length', 'keep-alive', 'transfer-encoding', 'upgrade', 'expect']);

function forwardable(headers: Headers) {
  return [...headers].filter(([name]) => !HOP_HEADERS.has(name.toLowerCase()));
}

function logApi(method: string, path: string, status: number, elapsedMs: number) {
  const line = `[proxy] ${method} ${path} -> ${status} (${elapsedMs}ms)`;
  if (status >= 500) console.error(line);
  else if (status >= 400) console.warn(line);
  else console.info(line);
}

export const handle: Handle = async ({ event, resolve }) => {
  if (!event.url.pathname.startsWith('/api/')) return resolve(event);

  const init: RequestInit = {
    method: event.request.method,
    headers: forwardable(event.request.headers)
  };
  // body 走缓冲而非流转发:分片上传单请求体 ≤ 分片大小(默认 5MB),
  // legacy 单发路径整体受 50MB 上限约束,均可整体读入,绕开 undici 流式 body 的 duplex 约束。
  // 响应方向原样透传:分段下载的 206/Content-Range 语义不受影响。
  if (!['GET', 'HEAD'].includes(event.request.method)) {
    try {
      init.body = await event.request.arrayBuffer();
    } catch (error) {
      // adapter-node 的 BODY_SIZE_LIMIT(默认 512KB)超限时,读请求体在这里抛 413;
      // 记日志并按错误 envelope 返回,让页面拿到中文提示而不是无信息的裸 413。
      const status = (error as { status?: number }).status;
      console.warn(
        `[proxy] ${event.request.method} ${event.url.pathname} 读取请求体失败 ` +
          `content-length=${event.request.headers.get('content-length')} status=${status ?? '?'}`,
        error
      );
      if (status === 413) {
        return new Response(
          JSON.stringify({ data: { code: 'payload_too_large', message: '请求体过大：超出前端容器 BODY_SIZE_LIMIT 配置' } }),
          { status: 413, headers: { 'content-type': 'application/json' } }
        );
      }
      throw error;
    }
  }

  const startedAt = Date.now();
  try {
    const response = await fetch(upstream() + event.url.pathname + event.url.search, init);
    logApi(event.request.method, event.url.pathname + event.url.search, response.status, Date.now() - startedAt);
    return new Response(response.body, {
      status: response.status,
      statusText: response.statusText,
      headers: forwardable(response.headers)
    });
  } catch (error) {
    console.error(`[proxy] ${event.request.method} ${event.url.pathname} 转发失败`, (error as { cause?: unknown })?.cause ?? error);
    logApi(event.request.method, event.url.pathname + event.url.search, 502, Date.now() - startedAt);
    // 错误体用 { data: { code, message } } 形状,client.ts 的 parseError 才能把中文原因展示出来。
    return new Response(JSON.stringify({ data: { code: 'bad_gateway', message: '后端服务不可达' } }), {
      status: 502,
      headers: { 'content-type': 'application/json' }
    });
  }
};

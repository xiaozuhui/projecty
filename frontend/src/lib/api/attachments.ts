import { API_BASE_URL, apiGet, apiPost, apiPutBinary, apiUpload, ApiClientError } from './client';
import type { Attachment, UploadSession } from './types';

const key = (value: string) => encodeURIComponent(value);

/// 与后端写死的上传上限(backend/src/config.rs UPLOAD_MAX_BYTES)保持一致,超限直接前端拦截。
export const MAX_ATTACHMENT_BYTES = 52_428_800;
/// 分段下载的单段大小与并发数:大文件按段拉取并展示进度。
const DOWNLOAD_SEGMENT_BYTES = 8 * 1024 * 1024;
const DOWNLOAD_CONCURRENCY = 2;
/// 单分片上传最大尝试次数(含首发),指数退避。
const CHUNK_MAX_ATTEMPTS = 4;

export function listTaskAttachments(taskKey: string) {
  return apiGet<Attachment[]>(`/tasks/${key(taskKey)}/attachments`);
}

export function uploadTaskAttachment(taskKey: string, file: File) {
  const form = new FormData();
  form.append('file', file);
  return apiUpload<Attachment>(`/tasks/${key(taskKey)}/attachments`, form);
}

export function deleteAttachment(attachmentId: string, reason?: string) {
  return apiPost<{ message: string }>(`/attachments/${key(attachmentId)}/delete`, reason ? { reason } : {});
}

/// 后端返回相对路径 /attachments/{object_key}/content,拼上 API 前缀供 <img> 直接使用。
export function attachmentUrl(path: string) {
  return `${API_BASE_URL}${path}`;
}

// ---- 分片上传 ----

export function initAttachmentUpload(
  taskKey: string,
  input: { file_name: string; mime_type: string | null; total_bytes: number; client_file_key: string; sha256?: string }
) {
  return apiPost<UploadSession>(`/tasks/${key(taskKey)}/attachments/uploads`, input);
}

export function getAttachmentUploadSession(uploadId: string) {
  return apiGet<UploadSession>(`/attachments/uploads/${key(uploadId)}`);
}

export function putAttachmentChunk(uploadId: string, index: number, chunk: Blob, sha256Hex: string, signal?: AbortSignal) {
  return apiPutBinary<UploadSession>(
    `/attachments/uploads/${key(uploadId)}/chunks/${index}`,
    chunk,
    { 'X-Checksum-Sha256': sha256Hex },
    signal
  );
}

export function completeAttachmentUpload(uploadId: string) {
  return apiPost<Attachment>(`/attachments/uploads/${key(uploadId)}/complete`, {});
}

export function abortAttachmentUpload(uploadId: string) {
  return apiPost<{ message: string }>(`/attachments/uploads/${key(uploadId)}/abort`, {});
}

export async function sha256Hex(buffer: ArrayBuffer): Promise<string> {
  const digest = await crypto.subtle.digest('SHA-256', buffer);
  return Array.from(new Uint8Array(digest), (byte) => byte.toString(16).padStart(2, '0')).join('');
}

/// 断点续传匹配指纹:后端限制 ≤128 个可打印 ASCII 字符,超长时折叠为 FNV-1a 哈希。
export function fileFingerprint(file: File): string {
  const raw = `${file.size}:${file.lastModified}:${encodeURIComponent(file.name)}`;
  if (raw.length <= 128 && /^[\x21-\x7e]+$/.test(raw)) return raw;
  let hash = 0x811c9dc5;
  for (let i = 0; i < raw.length; i += 1) {
    hash ^= raw.charCodeAt(i);
    hash = Math.imul(hash, 0x01000193) >>> 0;
  }
  return `fnv1a-${hash.toString(16)}-${raw.length}`;
}

export type UploadProgress = { uploadedBytes: number; totalBytes: number; speedBps: number; retries: number };
export type UploadOptions = { signal?: AbortSignal; onProgress?: (progress: UploadProgress) => void; concurrency?: number };

/// 分片 PUT 返回 404(会话被清理)时的内部信号:外层重建会话后重新调度。
class ChunkSessionReset extends Error {}

function isRetryableChunkError(error: unknown): boolean {
  // 网络中断/5xx/分片校验失败(400 + 指定文案)均可重试;权限、会话过期等不重试。
  if (error instanceof ApiClientError) {
    if (error.status >= 500) return true;
    return error.status === 400 && error.message.includes('分片校验失败');
  }
  return true;
}

/// 分片上传:并发上传 + 单片重试 + 会话级断点续传(刷新页面后重选同一文件可从已传分片继续)。
export async function uploadTaskAttachmentResumable(taskKey: string, file: File, options: UploadOptions = {}): Promise<Attachment> {
  if (file.size === 0) throw new Error('上传文件不能为空');
  if (file.size > MAX_ATTACHMENT_BYTES) throw new Error('文件大小不能超过 50 MB');
  const startedAt = performance.now();
  let retries = 0;

  const emit = (uploadedBytes: number) => {
    if (!options.onProgress) return;
    const elapsed = (performance.now() - startedAt) / 1000;
    options.onProgress({ uploadedBytes, totalBytes: file.size, speedBps: elapsed > 0 ? uploadedBytes / elapsed : 0, retries });
  };

  const initSession = () =>
    initAttachmentUpload(taskKey, {
      file_name: file.name,
      mime_type: file.type || null,
      total_bytes: file.size,
      client_file_key: fileFingerprint(file),
      sha256: fileSha256
    });

  // 整文件 sha256 在 complete 时由服务端核对,只算一次。
  const fileSha256 = await sha256Hex(await file.arrayBuffer());
  let session = (await initSession()).data;

  // 会话可能中途重建,分片长度始终按当前会话的 chunk_size 计算。
  const chunkLength = (index: number) => Math.min(session.chunk_size, file.size - index * session.chunk_size);
  let received = new Set(session.received_chunks);
  let baseBytes = [...received].reduce((sum, index) => sum + chunkLength(index), 0);

  const putChunk = async (index: number): Promise<void> => {
    const blob = file.slice(index * session.chunk_size, index * session.chunk_size + chunkLength(index));
    const checksum = await sha256Hex(await blob.arrayBuffer());
    for (let attempt = 0; ; attempt += 1) {
      if (options.signal?.aborted) throw new DOMException('上传已取消', 'AbortError');
      try {
        await putAttachmentChunk(session.upload_id, index, blob, checksum, options.signal);
        return;
      } catch (error) {
        if (error instanceof DOMException && error.name === 'AbortError') throw error;
        if (error instanceof ApiClientError && error.status === 404) {
          // 会话被清理(如超过 24h):重建会话,外层重新调度(再丢一次才失败)。
          session = (await initSession()).data;
          received = new Set(session.received_chunks);
          baseBytes = [...received].reduce((sum, i) => sum + chunkLength(i), 0);
          throw new ChunkSessionReset();
        }
        if (attempt >= CHUNK_MAX_ATTEMPTS - 1 || !isRetryableChunkError(error)) throw error;
        retries += 1;
        await new Promise((resolve) => setTimeout(resolve, 400 * 2 ** attempt + Math.random() * 200));
      }
    }
  };

  for (let round = 0; ; round += 1) {
    try {
      let cursor = 0;
      let doneBytes = 0;
      const pending = Array.from({ length: session.total_chunks }, (_, i) => i).filter((i) => !received.has(i));
      emit(baseBytes);
      const worker = async () => {
        for (;;) {
          const index = cursor;
          cursor += 1;
          if (index >= pending.length) return;
          await putChunk(pending[index]);
          doneBytes += chunkLength(pending[index]);
          emit(baseBytes + doneBytes);
        }
      };
      const lanes = Math.max(1, Math.min(options.concurrency ?? 3, pending.length || 1));
      await Promise.all(Array.from({ length: lanes }, worker));
      try {
        return (await completeAttachmentUpload(session.upload_id)).data;
      } catch (error) {
        // complete 报分片缺失(磁盘与回执不一致):查会话补传缺失片后重试一次。
        const message = error instanceof ApiClientError ? error.message : '';
        if (round === 0 && message.includes('分片缺失')) {
          received = new Set((await getAttachmentUploadSession(session.upload_id)).data.received_chunks);
          baseBytes = [...received].reduce((sum, i) => sum + chunkLength(i), 0);
          continue;
        }
        throw error;
      }
    } catch (error) {
      if (error instanceof ChunkSessionReset) {
        if (round >= 1) throw new Error('上传会话已失效，请稍后重试');
        continue;
      }
      if (error instanceof DOMException && error.name === 'AbortError') {
        await abortAttachmentUpload(session.upload_id).catch(() => undefined);
        throw error;
      }
      // 其他失败保留服务端会话:稍后重选同一文件可断点续传。
      throw error;
    }
  }
}

// ---- 分段下载 ----

export type DownloadProgress = { receivedBytes: number; totalBytes: number };

/// 分段下载:按 Range 分段并发拉取、边收边报进度,拼装后触发浏览器保存。
/// 附件内容按不可猜 object_key 公开,无需 Authorization,走同源代理。
export async function downloadAttachmentSegmented(
  url: string,
  fileName: string,
  totalBytes: number,
  options: { signal?: AbortSignal; onProgress?: (progress: DownloadProgress) => void } = {}
): Promise<void> {
  const buffer = new Uint8Array(totalBytes);
  let receivedBytes = 0;
  const segments: Array<[number, number]> = [];
  for (let start = 0; start < totalBytes; start += DOWNLOAD_SEGMENT_BYTES) {
    segments.push([start, Math.min(start + DOWNLOAD_SEGMENT_BYTES, totalBytes) - 1]);
  }
  const emit = () => options.onProgress?.({ receivedBytes, totalBytes });

  let cursor = 0;
  const worker = async () => {
    for (;;) {
      const current = cursor;
      cursor += 1;
      if (current >= segments.length) return;
      const [start, end] = segments[current];
      let response: Response;
      try {
        response = await fetch(url, { headers: { Range: `bytes=${start}-${end}` }, signal: options.signal });
      } catch (error) {
        if (error instanceof DOMException && error.name === 'AbortError') throw error;
        throw new Error('附件下载失败，请重试');
      }
      if (!response.ok && response.status !== 206) throw new Error('附件下载失败，请重试');
      // 兜底:服务器未按 Range 回 200 全量时,只取本段范围内的字节。
      const reader = response.body?.getReader();
      if (!reader) throw new Error('附件下载失败，请重试');
      let offset = start;
      if (response.status === 200) {
        // 丢弃 200 响应中本段之前的字节(理论上仅出现在不支持 Range 的部署)。
        let skip = start;
        while (skip > 0) {
          const { done, value } = await reader.read();
          if (done || !value) break;
          skip -= Math.min(skip, value.length);
        }
      }
      for (;;) {
        const { done, value } = await reader.read();
        if (done || !value) break;
        const take = Math.min(value.length, end + 1 - offset);
        buffer.set(value.subarray(0, take), offset);
        offset += take;
        receivedBytes += take;
        emit();
      }
      if (offset < end + 1) throw new Error('附件下载中断，请重试');
    }
  };
  await Promise.all(Array.from({ length: Math.min(DOWNLOAD_CONCURRENCY, segments.length || 1) }, worker));

  const blobUrl = URL.createObjectURL(new Blob([buffer]));
  const anchor = document.createElement('a');
  anchor.href = blobUrl;
  anchor.download = fileName;
  anchor.click();
  URL.revokeObjectURL(blobUrl);
}

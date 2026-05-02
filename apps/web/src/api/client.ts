// Empty string → relative URLs (frontend served from same origin as the API).
// Set VITE_SERVER_URL=http://localhost:4000 for the separate dev-server workflow.
export const SERVER_URL: string = import.meta.env.VITE_SERVER_URL ?? '';

// Optional Bearer token for the REST API. Must match ATLAS_API_TOKEN on the server.
const API_TOKEN: string = import.meta.env.VITE_API_TOKEN ?? '';

export interface ApiResponse<T> {
  status: 'success' | 'error';
  data?: T;
  message?: string;
  error?: string;
  count?: number;
}

export class ApiError extends Error {
  status: number;
  constructor(status: number, message: string) {
    super(message);
    this.name = 'ApiError';
    this.status = status;
  }
}

interface RequestOptions {
  method?: 'GET' | 'POST' | 'PATCH' | 'DELETE' | 'PUT';
  body?: unknown;
  signal?: AbortSignal;
  headers?: Record<string, string>;

  timeoutMs?: number;
}

const DEFAULT_TIMEOUT_MS = 30_000;

function withTimeout(signal: AbortSignal | undefined, timeoutMs: number): AbortSignal {
  if (timeoutMs <= 0) return signal ?? new AbortController().signal;
  const ctrl = new AbortController();
  const timer = setTimeout(() => ctrl.abort(new Error(`Request timed out after ${timeoutMs}ms`)), timeoutMs);
  if (signal) {
    if (signal.aborted) ctrl.abort(signal.reason);
    else signal.addEventListener('abort', () => ctrl.abort(signal.reason), { once: true });
  }
  ctrl.signal.addEventListener('abort', () => clearTimeout(timer), { once: true });
  return ctrl.signal;
}

async function request<T>(path: string, opts: RequestOptions = {}): Promise<T> {
  const url = path.startsWith('http') ? path : `${SERVER_URL}${path}`;
  const init: RequestInit = {
    method: opts.method ?? 'GET',
    headers: {
      'Content-Type': 'application/json',
      ...(API_TOKEN ? { 'Authorization': `Bearer ${API_TOKEN}` } : {}),
      ...(opts.headers ?? {}),
    },
    signal: withTimeout(opts.signal, opts.timeoutMs ?? DEFAULT_TIMEOUT_MS),
  };
  if (opts.body !== undefined) {
    init.body = JSON.stringify(opts.body);
  }

  const res = await fetch(url, init);
  if (res.status === 204) return undefined as T;

  let payload: ApiResponse<T> | null = null;
  try {
    payload = (await res.json()) as ApiResponse<T>;
  } catch {
    payload = null;
  }

  if (!res.ok) {
    const msg = payload?.message ?? payload?.error ?? `HTTP ${res.status}`;
    throw new ApiError(res.status, msg);
  }
  if (payload?.status === 'error') {
    throw new ApiError(res.status, payload.message ?? payload.error ?? 'API error');
  }

  return (payload?.data ?? (payload as unknown as T)) as T;
}

async function requestForm<T>(path: string, form: FormData): Promise<T> {
  const url = path.startsWith('http') ? path : `${SERVER_URL}${path}`;
  const res = await fetch(url, { method: 'POST', body: form });
  let payload: ApiResponse<T> | null = null;
  try { payload = (await res.json()) as ApiResponse<T>; } catch { payload = null; }
  if (!res.ok) throw new ApiError(res.status, payload?.message ?? payload?.error ?? `HTTP ${res.status}`);
  if (payload?.status === 'error') throw new ApiError(res.status, payload.message ?? payload.error ?? 'API error');
  return (payload?.data ?? (payload as unknown as T)) as T;
}

export const api = {
  get: <T>(path: string, opts?: Omit<RequestOptions, 'method' | 'body'>) =>
    request<T>(path, { ...opts, method: 'GET' }),
  post: <T>(path: string, body?: unknown, opts?: Omit<RequestOptions, 'method' | 'body'>) =>
    request<T>(path, { ...opts, method: 'POST', body }),
  patch: <T>(path: string, body?: unknown, opts?: Omit<RequestOptions, 'method' | 'body'>) =>
    request<T>(path, { ...opts, method: 'PATCH', body }),
  put: <T>(path: string, body?: unknown, opts?: Omit<RequestOptions, 'method' | 'body'>) =>
    request<T>(path, { ...opts, method: 'PUT', body }),
  delete: <T = void>(path: string, opts?: Omit<RequestOptions, 'method' | 'body'>) =>
    request<T>(path, { ...opts, method: 'DELETE' }),
  postForm: <T>(path: string, form: FormData) =>
    requestForm<T>(path, form),
};

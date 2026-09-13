import { monotonicNowMs } from './clock.js';
import { SessionTelemetry } from './index.js';

export interface NormalizeUrlOptions {
  // Query parameter names to keep verbatim. Everything else is stripped by default, since
  // query-string values often carry auth tokens, session ids, or other secrets.
  allowlistedQueryParams?: readonly string[];
}

// Paths remain visible (applications must not put secrets there). Only HTTP(S) URLs are
// recorded; credentials, fragments, and non-allowlisted query values never reach the buffer.
export function normalizeUrl(url: string, options: NormalizeUrlOptions = {}): string {
  try {
    // RN's URL implementation has read-only credential/fragment fields. Redact directly
    // without depending on browser-only URL setters.
    const withoutFragment = url.split('#', 1)[0]!;
    const queryIndex = withoutFragment.indexOf('?');
    let base = queryIndex < 0 ? withoutFragment : withoutFragment.slice(0, queryIndex);
    const query = queryIndex < 0 ? '' : withoutFragment.slice(queryIndex + 1);
    if (/^[a-z][a-z\d+.-]*:/i.test(base) && !/^https?:\/\//i.test(base)) {
      return '[redacted-url]';
    }
    if (/\s|\\/.test(base)) return '[invalid-url]';
    base = base.replace(
      /^(https?:\/\/|\/\/)([^/]*)(.*)$/i,
      (_match, scheme: string, authority: string, path: string) => {
        const host = authority.slice(authority.lastIndexOf('@') + 1);
        if (!host) throw new Error('Missing host');
        return scheme.toLowerCase() + host.toLowerCase() + (path || '/');
      },
    );
    const allowlist = new Set(options.allowlistedQueryParams ?? []);
    const kept = query.split('&').filter((pair) => {
      const key = pair.split('=', 1)[0]!;
      return key !== '' && allowlist.has(decodeURIComponent(key.replace(/\+/g, ' ')));
    });
    return base + (kept.length ? '?' + kept.join('&') : '');
  } catch {
    return '[invalid-url]';
  }
}

function resolveRequestUrl(input: RequestInfo | URL): string {
  if (typeof input === 'string') {
    return input;
  }
  return input instanceof URL ? input.toString() : input.url;
}

function estimateBodyBytes(body: BodyInit_ | null | undefined): number | null {
  if (body == null) {
    return null;
  }
  if (typeof body === 'string' && typeof TextEncoder !== 'undefined') {
    return new TextEncoder().encode(body).length;
  }
  if (typeof Blob !== 'undefined' && body instanceof Blob) {
    return body.size;
  }
  if (body instanceof ArrayBuffer) {
    return body.byteLength;
  }
  if (ArrayBuffer.isView(body)) {
    return body.byteLength;
  }
  // FormData/ReadableStream/URLSearchParams: no reliable byte size without consuming the
  // stream, which this wrapper must not do (it would break the actual request).
  return null;
}

function parseContentLength(raw: string | null): number | null {
  if (raw === null || !/^\d+$/.test(raw.trim())) return null;
  const parsed = Number(raw);
  return Number.isSafeInteger(parsed) && parsed >= 0 ? parsed : null;
}

// Observational code must never change a request's success, failure, or event delivery.
function observe(action: () => void): void {
  try {
    action();
  } catch {
    // Missing runtime utilities or a failing telemetry bridge disable this observation only.
  }
}

type NetworkRecorder = typeof SessionTelemetry.recordNetworkRequest;
let automaticCaptureActive = false;

// The pinned RN fetch polyfill and XHR clients share this public transport boundary. Do not
// also patch fetch: doing so would emit two events for one request. No RN private interceptor
// is replaced, and application event handlers, upload/progress settings and bodies are untouched.
export function startNetworkCapture(
  record: NetworkRecorder,
  options: NormalizeUrlOptions = {},
): () => void {
  const proto = globalThis.XMLHttpRequest?.prototype;
  if (!proto) return () => {};
  const originalOpen = proto.open;
  const originalSend = proto.send;
  const requests = new WeakMap<XMLHttpRequest, { method: string; url: string }>();
  const pending = new WeakSet<XMLHttpRequest>();
  const cleanups = new Set<() => void>();
  let active = true;

  const open: typeof proto.open = function (this: XMLHttpRequest, ...args) {
    const result = originalOpen.apply(this, args);
    if (active) {
      observe(() => {
        requests.set(this, { method: args[0].toUpperCase(), url: normalizeUrl(args[1], options) });
      });
    }
    return result;
  };
  const send: typeof proto.send = function (this: XMLHttpRequest, ...args) {
    const request = active ? requests.get(this) : undefined;
    let cleanup = () => {};
    // Invalid repeated send() calls are left to XHR; they must not detach the original capture.
    if (request && !pending.has(this)) {
      observe(() => {
        const startedAt = monotonicNowMs();
        const requestBytes = estimateBodyBytes(args[0]);
        let finished = false;
        const terminalEvents = ['load', 'error', 'abort', 'timeout'] as const;
        const complete = (type: string) => {
          if (finished) return;
          finished = true;
          cleanup();
          if (!active) return;
          observe(() =>
            record(
              request.method,
              request.url,
              type === 'load' ? this.status : 0,
              Math.max(0, monotonicNowMs() - startedAt),
              requestBytes,
              type === 'load' ? parseContentLength(this.getResponseHeader('content-length')) : null,
            ),
          );
        };
        const listeners = terminalEvents.map((type) => ({ type, listener: () => complete(type) }));
        cleanup = () => {
          for (const { type, listener } of listeners)
            observe(() => this.removeEventListener(type, listener));
          pending.delete(this);
          cleanups.delete(cleanup);
        };
        cleanups.add(cleanup);
        pending.add(this);
        try {
          for (const { type, listener } of listeners) this.addEventListener(type, listener);
        } catch {
          cleanup();
        }
      });
    }
    try {
      return originalSend.apply(this, args);
    } catch (error) {
      cleanup();
      throw error;
    }
  };

  // Some hosts expose immutable transport methods. Roll back partial installation there.
  try {
    proto.open = open;
    proto.send = send;
  } catch {
    observe(() => {
      if (proto.open === open) proto.open = originalOpen;
    });
    return () => {};
  }
  automaticCaptureActive = true;
  return () => {
    active = false;
    automaticCaptureActive = false;
    for (const cleanup of cleanups) cleanup();
    // A later third-party patch belongs to its owner. Our retained wrappers become pass-through.
    observe(() => {
      if (proto.open === open) proto.open = originalOpen;
    });
    observe(() => {
      if (proto.send === send) proto.send = originalSend;
    });
  };
}

// Legacy explicit fetch helper. While automatic XHR capture is active it is a pass-through,
// so existing integrations do not double-count. Independent non-XHR fetch implementations
// are outside automatic capture's support boundary.
export function createInstrumentedFetch(
  baseFetch: typeof fetch = fetch,
  options: NormalizeUrlOptions = {},
): typeof fetch {
  return function (this: unknown, input, init) {
    if (automaticCaptureActive) return baseFetch.call(this, input, init);
    let method = 'GET';
    let url = '[invalid-url]';
    let requestBytes: number | null = null;
    let start = 0;
    observe(() => {
      method = (
        init?.method ?? (typeof input === 'object' && 'method' in input ? input.method : 'GET')
      ).toUpperCase();
      url = normalizeUrl(resolveRequestUrl(input), options);
      requestBytes = estimateBodyBytes(init?.body);
      start = monotonicNowMs();
    });
    const result = baseFetch.call(this, input, init);
    return result.then(
      (response) => {
        observe(() =>
          SessionTelemetry.recordNetworkRequest(
            method,
            url,
            response.status,
            monotonicNowMs() - start,
            requestBytes,
            parseContentLength(response.headers.get('content-length')),
          ),
        );
        return response;
      },
      (error) => {
        observe(() =>
          SessionTelemetry.recordNetworkRequest(
            method,
            url,
            0,
            monotonicNowMs() - start,
            requestBytes,
            null,
          ),
        );
        throw error;
      },
    );
  };
}

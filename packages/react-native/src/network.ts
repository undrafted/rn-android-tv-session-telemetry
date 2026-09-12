import { monotonicNowMs } from './clock.js';
import { SessionTelemetry } from './index.js';

export interface NormalizeUrlOptions {
  // Query parameter names to keep verbatim. Everything else is stripped — plan.md section 6's
  // privacy defaults: "Query-string values unless explicitly allowlisted" are not captured.
  allowlistedQueryParams?: readonly string[];
}

// Scoped to query-string redaction only — the concrete, common leak vector (auth tokens,
// session ids, API keys as query params) for a network *request URL* specifically. Doesn't
// touch the path (no reliable way to auto-detect a secret embedded there without a real network
// wrapper's context) or headers (out of scope for a URL-only utility). "Secrets are redacted"
// more broadly is a larger feature that lands once the network capture wrapper itself exists.
export function normalizeUrl(url: string, options: NormalizeUrlOptions = {}): string {
  const allowlist = new Set(options.allowlistedQueryParams ?? []);

  let parsed: URL;
  try {
    parsed = new URL(url);
  } catch {
    // Not a valid absolute URL (relative path, malformed input, etc.) — recording network
    // activity shouldn't crash on it, so return it unchanged rather than throwing.
    return url;
  }

  for (const key of [...parsed.searchParams.keys()]) {
    if (!allowlist.has(key)) {
      parsed.searchParams.delete(key);
    }
  }

  return parsed.toString();
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
  if (typeof body === 'string') {
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

function parseContentLength(headers: Headers): number | null {
  const raw = headers.get('content-length');
  if (raw === null) {
    return null;
  }
  const parsed = Number(raw);
  return Number.isFinite(parsed) ? parsed : null;
}

// Wraps fetch to record method/normalized-URL/status/duration/byte-counts per plan.md
// section 6's Network row. Explicit opt-in (the app passes its own fetch and uses the
// returned wrapped version) rather than monkey-patching the global — matches this library's
// existing preference for explicit markers over patching internals (plan.md section 8.1).
// Only covers fetch: plan.md's own risk list already documents that "network wrapping misses
// native clients" as a known V1 limitation, not something this closes.
export function createInstrumentedFetch(
  baseFetch: typeof fetch = fetch,
  options: NormalizeUrlOptions = {},
): typeof fetch {
  return async (input, init) => {
    const method = (init?.method ?? 'GET').toUpperCase();
    const url = normalizeUrl(resolveRequestUrl(input), options);
    const requestBytes = estimateBodyBytes(init?.body);
    const start = monotonicNowMs();

    try {
      const response = await baseFetch(input, init);
      SessionTelemetry.recordNetworkRequest(
        method,
        url,
        response.status,
        monotonicNowMs() - start,
        requestBytes,
        parseContentLength(response.headers),
      );
      return response;
    } catch (error) {
      SessionTelemetry.recordNetworkRequest(
        method,
        url,
        0,
        monotonicNowMs() - start,
        requestBytes,
        null,
      );
      throw error;
    }
  };
}

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

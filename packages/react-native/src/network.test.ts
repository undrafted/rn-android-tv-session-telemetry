import { beforeEach, describe, expect, it, vi } from 'vitest';
import { normalizeUrl } from './network.js';

describe('normalizeUrl', () => {
  it('strips query params by default', () => {
    expect(normalizeUrl('https://api.example.com/program/482?token=secret&page=2')).toBe(
      'https://api.example.com/program/482',
    );
  });

  it('keeps allowlisted query params and strips the rest', () => {
    expect(
      normalizeUrl('https://api.example.com/search?q=star+wars&session=abc123', {
        allowlistedQueryParams: ['q'],
      }),
    ).toBe('https://api.example.com/search?q=star+wars');
  });

  it('leaves a URL with no query string unchanged', () => {
    expect(normalizeUrl('https://api.example.com/program/482')).toBe(
      'https://api.example.com/program/482',
    );
  });

  it('strips every value for a repeated non-allowlisted param', () => {
    expect(normalizeUrl('https://api.example.com/items?tag=a&tag=b')).toBe(
      'https://api.example.com/items',
    );
  });

  it('returns a malformed/relative URL unchanged rather than throwing', () => {
    expect(normalizeUrl('/relative/path?token=secret')).toBe('/relative/path?token=secret');
    expect(() => normalizeUrl('not a url at all')).not.toThrow();
  });
});

const { recordNetworkRequestMock } = vi.hoisted(() => ({
  recordNetworkRequestMock: vi.fn(),
}));

vi.mock('./index.js', () => ({
  SessionTelemetry: { recordNetworkRequest: recordNetworkRequestMock },
}));

const { createInstrumentedFetch } = await import('./network.js');

function jsonResponse(status: number, headers: Record<string, string> = {}): Response {
  return new Response('{}', { status, headers });
}

beforeEach(() => {
  recordNetworkRequestMock.mockClear();
});

describe('createInstrumentedFetch', () => {
  it('records method, normalized URL, status, and response byte count on success', async () => {
    const baseFetch = vi.fn().mockResolvedValue(jsonResponse(200, { 'content-length': '42' }));
    const fetchImpl = createInstrumentedFetch(baseFetch);

    await fetchImpl('https://api.example.com/program/482?token=secret');

    expect(baseFetch).toHaveBeenCalledTimes(1);
    expect(recordNetworkRequestMock).toHaveBeenCalledTimes(1);
    const [method, url, status, durationMs, requestBytes, responseBytes] = recordNetworkRequestMock
      .mock.calls[0] as [string, string, number, number, unknown, number];
    expect(method).toBe('GET');
    expect(url).toBe('https://api.example.com/program/482');
    expect(status).toBe(200);
    expect(durationMs).toBeGreaterThanOrEqual(0);
    expect(requestBytes).toBeNull();
    expect(responseBytes).toBe(42);
  });

  it('uppercases the method and estimates request body bytes for a POST', async () => {
    const baseFetch = vi.fn().mockResolvedValue(jsonResponse(201));
    const fetchImpl = createInstrumentedFetch(baseFetch);

    await fetchImpl('https://api.example.com/items', {
      method: 'post',
      body: 'hello',
    });

    const [method, , , , requestBytes] = recordNetworkRequestMock.mock.calls[0] as [
      string,
      string,
      number,
      number,
      number | null,
    ];
    expect(method).toBe('POST');
    expect(requestBytes).toBe(5);
  });

  it('records status 0 and rethrows when the underlying fetch rejects', async () => {
    const baseFetch = vi.fn().mockRejectedValue(new Error('network down'));
    const fetchImpl = createInstrumentedFetch(baseFetch);

    await expect(fetchImpl('https://api.example.com/items')).rejects.toThrow('network down');

    const [, , status, , , responseBytes] = recordNetworkRequestMock.mock.calls[0] as [
      string,
      string,
      number,
      number,
      number | null,
      number | null,
    ];
    expect(status).toBe(0);
    expect(responseBytes).toBeNull();
  });
});

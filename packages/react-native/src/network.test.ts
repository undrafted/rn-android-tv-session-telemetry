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

  it.each([
    ['https://user:password@example.com/items?token=secret#secret', 'https://example.com/items'],
    ['//user:password@example.com/items?token=secret#secret', '//example.com/items'],
    ['/items?token=secret#secret', '/items'],
    ['https://example.com?token=secret', 'https://example.com/'],
    ['data:text/plain,secret', '[redacted-url]'],
    ['https://example.com/items?%zz=secret', '[invalid-url]'],
    ['http://user:secret@/items', '[invalid-url]'],
  ])('redacts %s', (input, expected) => {
    expect(normalizeUrl(input)).toBe(expected);
  });

  it('preserves repeated allowlisted values, including encoded names', () => {
    expect(
      normalizeUrl('/items?%70age=1&page=2&token=secret#secret', {
        allowlistedQueryParams: ['page'],
      }),
    ).toBe('/items?%70age=1&page=2');
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

  it('redacts relative query strings and safely handles malformed URLs', () => {
    expect(normalizeUrl('/relative/path?token=secret')).toBe('/relative/path');
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

it('uses Request.method and preserves response identity in the legacy helper', async () => {
  const response = jsonResponse(200);
  const baseFetch = vi.fn().mockResolvedValue(response);
  const input = new Request('https://example.com/items', { method: 'POST', body: 'secret' });
  expect(await createInstrumentedFetch(baseFetch)(input)).toBe(response);
  expect(baseFetch).toHaveBeenCalledWith(input, undefined);
  expect(recordNetworkRequestMock.mock.calls[0]?.[0]).toBe('POST');
});

it.each(['-1', '1.5', 'NaN', '', '9007199254740992'])(
  'does not report invalid byte counts: %s',
  async (raw) => {
    await createInstrumentedFetch(
      vi.fn().mockResolvedValue(jsonResponse(200, { 'content-length': raw })),
    )('https://example.com/items');
    expect(recordNetworkRequestMock.mock.calls[0]?.[5]).toBeNull();
  },
);

it('keeps the original error when the legacy recorder fails', async () => {
  const error = new Error('network error');
  recordNetworkRequestMock.mockImplementationOnce(() => {
    throw new Error('telemetry error');
  });
  await expect(
    createInstrumentedFetch(vi.fn().mockRejectedValue(error))('https://example.com'),
  ).rejects.toBe(error);
});

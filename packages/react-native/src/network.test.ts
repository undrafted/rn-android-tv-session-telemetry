import { describe, expect, it } from 'vitest';
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

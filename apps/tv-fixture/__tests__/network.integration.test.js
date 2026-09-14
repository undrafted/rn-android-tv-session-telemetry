/** @format */
/* eslint-disable @react-native/no-deep-imports -- Integration tests exercise the real pinned transport. */
import { Buffer } from 'node:buffer';

// Exercise the pinned RN XMLHttpRequest implementation and its real whatwg-fetch client.
// Only the native networking boundary and writer are substituted; request/event semantics,
// URL polyfills, fetch Request/Response handling, and telemetry installation are real.
import XMLHttpRequest from 'react-native/Libraries/Network/XMLHttpRequest';
import RCTNetworking from 'react-native/Libraries/Network/RCTNetworking';
import { URL as NativeURL } from 'react-native/Libraries/Blob/URL';
import { fetch as rnFetch, Request } from 'whatwg-fetch';
import { NativeModules } from 'react-native';
import {
  SessionTelemetry,
  createInstrumentedFetch,
} from '@rn-android-tv-session-telemetry/react-native';

jest.mock('react-native/Libraries/Network/RCTNetworking', () => {
  const listeners = new Map();
  let nextId = 1;
  const requests = new Map();
  return {
    __esModule: true,
    default: {
      requests,
      get lastId() {
        return nextId - 1;
      },
      addListener: (name, callback) => {
        const set = listeners.get(name) ?? new Set();
        listeners.set(name, set);
        set.add(callback);
        return { remove: () => set.delete(callback) };
      },
      emit: (name, args) => {
        for (const callback of [...(listeners.get(name) ?? [])]) callback(args);
      },
      sendRequest: jest.fn((...args) => {
        const id = nextId++;
        requests.set(id, args);
        args[8](id);
      }),
      abortRequest: jest.fn(),
    },
  };
});

jest.mock('react-native', () => ({
  TVEventHandler: { addListener: () => ({ remove() {} }) },
  Platform: { OS: 'android', constants: { Model: 'test-tv' }, Version: 36 },
  NativeEventEmitter: class {
    addListener() {
      return { remove() {} };
    }
  },
  NativeModules: {
    RNSessionTelemetryWriter: {
      start: jest.fn(),
      finish: jest.fn(),
      pushEvent: jest.fn(),
    },
  },
}));

const originalXHR = global.XMLHttpRequest;
const originalFetch = global.fetch;
const originalURL = global.URL;
const open = XMLHttpRequest.prototype.open;
const send = XMLHttpRequest.prototype.send;

beforeEach(() => {
  global.XMLHttpRequest = XMLHttpRequest;
  global.fetch = rnFetch;
  global.URL = NativeURL;
  jest.clearAllMocks();
});
afterEach(() => {
  SessionTelemetry.stop();
  global.XMLHttpRequest = originalXHR;
  global.fetch = originalFetch;
  global.URL = originalURL;
});

function events() {
  return NativeModules.RNSessionTelemetryWriter.pushEvent.mock.calls
    .map(([json]) => JSON.parse(json))
    .filter(event => event.type === 'network');
}
function latestId() {
  return RCTNetworking.lastId;
}
function complete(
  id,
  status = 200,
  headers = { 'Content-Length': '2' },
  body = '{}',
) {
  RCTNetworking.emit('didReceiveNetworkResponse', [
    id,
    status,
    headers,
    'https://example.com/final',
  ]);
  const data =
    RCTNetworking.requests.get(id)[5] === 'base64'
      ? Buffer.from(body).toString('base64')
      : body;
  RCTNetworking.emit('didReceiveNetworkData', [id, data]);
  RCTNetworking.emit('didCompleteNetworkResponse', [id, '']);
}

it('records an ordinary fetch once after one startup install, preserving the request and body', async () => {
  SessionTelemetry.install();
  const responsePromise = fetch(
    'https://user:password@example.com/items?token=secret#private',
    {
      method: 'POST',
      body: 'request-secret',
      headers: { Authorization: 'Bearer secret' },
    },
  );
  const id = latestId();
  expect(RCTNetworking.sendRequest.mock.calls[0].slice(0, 5)).toEqual([
    'POST',
    undefined,
    'https://user:password@example.com/items?token=secret#private',
    expect.objectContaining({ authorization: 'Bearer secret' }),
    'request-secret',
  ]);
  complete(id, 201, { 'Content-Length': '15' }, 'response-secret');
  const response = await responsePromise;
  expect(response.status).toBe(201);
  expect(await response.text()).toBe('response-secret');
  expect(events()).toEqual([
    expect.objectContaining({
      method: 'POST',
      url: 'https://example.com/items',
      status: 201,
      responseBytes: 15,
    }),
  ]);
  expect(JSON.stringify(events())).not.toMatch(
    /secret|password|private|Authorization/,
  );
  expect(events()[0].durationMs).toBeGreaterThanOrEqual(0);
});

it('captures Request method overrides and leaves HTTP failures as fulfilled responses', async () => {
  SessionTelemetry.install({ network: { allowlistedQueryParams: ['page'] } });
  const request = new Request('https://example.com/items?page=2&token=secret', {
    method: 'POST',
    body: 'body',
  });
  const promise = fetch(request, { method: 'PUT' });
  complete(latestId(), 503);
  expect((await promise).status).toBe(503);
  expect(events()).toEqual([
    expect.objectContaining({
      method: 'PUT',
      url: 'https://example.com/items?page=2',
      status: 503,
    }),
  ]);
});

it('keeps XHR load/progress handlers, timeout, credentials and response identity intact', () => {
  SessionTelemetry.install();
  const xhr = new XMLHttpRequest();
  const onload = jest.fn();
  const onprogress = jest.fn();
  xhr.onload = onload;
  xhr.onprogress = onprogress;
  xhr.open('GET', 'https://example.com/items');
  xhr.timeout = 123;
  xhr.withCredentials = false;
  xhr.send();
  const id = latestId();
  expect(RCTNetworking.sendRequest.mock.calls[0][7]).toBe(123);
  expect(RCTNetworking.sendRequest.mock.calls[0][9]).toBe(false);
  RCTNetworking.emit('didReceiveNetworkDataProgress', [id, 1, 2]);
  complete(id);
  expect(onload).toHaveBeenCalledTimes(1);
  expect(onprogress).toHaveBeenCalledTimes(1);
  expect(xhr.onload).toBe(onload);
  expect(xhr.responseText).toBe('{}');
  expect(events()).toHaveLength(1);
});

it.each(['error', 'timeout', 'abort'])(
  'preserves %s delivery and records one status-zero event',
  async outcome => {
    SessionTelemetry.install();
    const xhr = new XMLHttpRequest();
    const handler = jest.fn();
    xhr.addEventListener(outcome, handler);
    xhr.open('GET', 'https://example.com/items');
    xhr.send();
    const id = latestId();
    if (outcome === 'abort') xhr.abort();
    else
      RCTNetworking.emit('didCompleteNetworkResponse', [
        id,
        'failed',
        outcome === 'timeout',
      ]);
    expect(handler).toHaveBeenCalledTimes(1);
    expect(events()).toEqual([
      expect.objectContaining({ status: 0, responseBytes: null }),
    ]);
  },
);

it('preserves fetch cancellation and rejection', async () => {
  SessionTelemetry.install();
  const controller = new AbortController();
  const promise = fetch('https://example.com/items', {
    signal: controller.signal,
  });
  latestId();
  controller.abort();
  await expect(promise).rejects.toHaveProperty('name', 'AbortError');
  expect(events()).toHaveLength(1);
});

it('does not capture before install or after stop, restores methods, and excludes old in-flight requests', () => {
  const before = new XMLHttpRequest();
  before.open('GET', 'https://example.com/before');
  before.send();
  const beforeId = latestId();
  SessionTelemetry.install();
  complete(beforeId);
  const old = new XMLHttpRequest();
  old.open('GET', 'https://example.com/old');
  old.send();
  const oldId = latestId();
  SessionTelemetry.install();
  complete(oldId);
  expect(events()).toHaveLength(0);
  SessionTelemetry.stop();
  expect(XMLHttpRequest.prototype.open).toBe(open);
  expect(XMLHttpRequest.prototype.send).toBe(send);
  const after = new XMLHttpRequest();
  after.open('GET', 'https://example.com/after');
  after.send();
  complete(latestId());
  expect(events()).toHaveLength(0);
});

it('avoids stacking on reinstall and double-counting the legacy fetch helper', async () => {
  SessionTelemetry.install();
  SessionTelemetry.install();
  const promise = createInstrumentedFetch(fetch)('https://example.com/items');
  complete(latestId());
  await promise;
  expect(events()).toHaveLength(1);
});

it('does not change request success when the telemetry bridge throws', async () => {
  SessionTelemetry.install();
  NativeModules.RNSessionTelemetryWriter.pushEvent.mockImplementationOnce(
    () => {
      throw new Error('bridge failed');
    },
  );
  const promise = fetch('https://example.com/items');
  complete(latestId());
  expect((await promise).status).toBe(200);
});

it('preserves synchronous invalid-send errors without adding an event or losing a pending request', () => {
  SessionTelemetry.install();
  const xhr = new XMLHttpRequest();
  expect(() => xhr.send()).toThrow('Request has not been opened');
  xhr.open('GET', 'https://example.com/items');
  xhr.send();
  const id = latestId();
  expect(() => xhr.send()).toThrow('Request has already been sent');
  complete(id);
  expect(events()).toHaveLength(1);
});

it('keeps concurrent request metadata separate when they finish out of order', async () => {
  SessionTelemetry.install();
  expect(global.fetch).toBe(rnFetch);
  const first = fetch('https://example.com/first?secret=a');
  const firstId = latestId();
  const second = fetch('https://example.com/second?secret=b');
  const secondId = latestId();
  complete(secondId, 202, {});
  complete(firstId, 200);
  expect((await first).status).toBe(200);
  expect((await second).status).toBe(202);
  expect(
    events().map(event => [event.url, event.status, event.responseBytes]),
  ).toEqual([
    ['https://example.com/second', 202, null],
    ['https://example.com/first', 200, 2],
  ]);
});

it('preserves later third-party patches and leaves retained telemetry wrappers inactive', () => {
  SessionTelemetry.install();
  const capturedSend = XMLHttpRequest.prototype.send;
  const thirdPartySend = function (...args) {
    return capturedSend.apply(this, args);
  };
  XMLHttpRequest.prototype.send = thirdPartySend;
  try {
    SessionTelemetry.stop();
    expect(XMLHttpRequest.prototype.send).toBe(thirdPartySend);
    const xhr = new XMLHttpRequest();
    xhr.open('GET', 'https://example.com/items');
    xhr.send();
    complete(latestId());
    expect(events()).toHaveLength(0);
  } finally {
    XMLHttpRequest.prototype.send = send;
  }
});

it('gracefully skips capture when XMLHttpRequest is unavailable', () => {
  global.XMLHttpRequest = undefined;
  expect(() => SessionTelemetry.install()).not.toThrow();
  expect(() => SessionTelemetry.stop()).not.toThrow();
  expect(global.fetch).toBe(rnFetch);
});

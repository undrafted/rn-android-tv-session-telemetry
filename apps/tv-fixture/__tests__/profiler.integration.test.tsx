/** @format */
import React, { useEffect, useState } from 'react';
import TestRenderer, { act } from 'react-test-renderer';
import { NativeModules } from 'react-native';
import {
  SessionTelemetry,
  withTelemetryRoot,
  type ReactCommitEvent,
} from '@rn-android-tv-session-telemetry/react-native';

jest.mock('react-native', () => ({
  TVEventHandler: { addListener: () => ({ remove() {} }) },
  Platform: { OS: 'android', constants: { Model: 'test-tv' }, Version: 36 },
  NativeEventEmitter: class {
    addListener() {
      return { remove() {} };
    }
  },
  NativeModules: {
    RNSessionTelemetryWriter: { start() {}, finish() {}, pushEvent: jest.fn() },
  },
}));

const writer = NativeModules.RNSessionTelemetryWriter;
let renderer: TestRenderer.ReactTestRenderer | undefined;
afterEach(async () => {
  SessionTelemetry.stop();
  if (renderer) await act(() => renderer!.unmount());
  renderer = undefined;
  jest.restoreAllMocks();
});
beforeEach(() => {
  writer.pushEvent.mockReset();
});
function commits(): ReactCommitEvent[] {
  return writer.pushEvent.mock.calls
    .map(([json]: [string]) => JSON.parse(json))
    .filter((event: { type: string }) => event.type === 'react-commit');
}

it('records mount and updates through the native writer without application Profiler callbacks', async () => {
  function App({ label }: { label: string }) {
    const [count, setCount] = useState(0);
    return (
      <button onClick={() => setCount(value => value + 1)}>
        {label}:{count}
      </button>
    );
  }
  const Root = withTelemetryRoot(App, 'fixture-root');
  SessionTelemetry.install();
  await act(() => {
    renderer = TestRenderer.create(<Root label="value" />);
  });
  expect(commits()).toHaveLength(1);
  expect(commits()[0]).toEqual({
    type: 'react-commit',
    sequence: expect.any(Number),
    timestamp: expect.any(Number),
    profilerId: 'fixture-root',
    phase: 'mount',
    actualDurationMs: expect.any(Number),
    baseDurationMs: expect.any(Number),
    renderStartMs: expect.any(Number),
    commitTimeMs: expect.any(Number),
  });
  await act(() => renderer!.root.findByType('button').props.onClick());
  expect(commits().map(event => event.phase)).toEqual(['mount', 'update']);
  for (const event of commits()) {
    expect(event.renderStartMs).toBeLessThanOrEqual(event.commitTimeMs!);
    expect(event.commitTimeMs).toBeLessThanOrEqual(event.timestamp);
  }
  expect(renderer!.root.findByType('button').children).toEqual([
    'value',
    ':',
    '1',
  ]);
  expect(JSON.stringify(commits())).not.toContain('value');
});

it('preserves component state and effects through stop and reinstall without duplicate capture', async () => {
  const mounted = jest.fn();
  const unmounted = jest.fn();
  function App() {
    const [count, setCount] = useState(0);
    useEffect(() => {
      mounted();
      return unmounted;
    }, []);
    return (
      <button onClick={() => setCount(value => value + 1)}>{count}</button>
    );
  }
  const Root = withTelemetryRoot(App);
  SessionTelemetry.install();
  await act(() => {
    renderer = TestRenderer.create(<Root />);
  });
  SessionTelemetry.stop();
  const before = commits().length;
  await act(() => renderer!.root.findByType('button').props.onClick());
  expect(commits()).toHaveLength(before);
  SessionTelemetry.install();
  SessionTelemetry.install();
  await act(() => renderer!.root.findByType('button').props.onClick());
  expect(commits()).toHaveLength(before + 1);
  expect(renderer!.root.findByType('button').children).toEqual(['2']);
  expect(mounted).toHaveBeenCalledTimes(1);
  expect(unmounted).not.toHaveBeenCalled();
});

it('does not record before install and keeps rendering when the writer fails', async () => {
  const Root = withTelemetryRoot(({ label }: { label: string }) => (
    <button>{label}</button>
  ));
  await act(() => {
    renderer = TestRenderer.create(<Root label="before" />);
  });
  expect(commits()).toHaveLength(0);
  SessionTelemetry.install();
  writer.pushEvent.mockImplementationOnce(() => {
    throw new Error('bridge failed');
  });
  await act(() => renderer!.update(<Root label="after" />));
  expect(renderer!.root.findByType('button').children).toEqual(['after']);
});

it('forwards refs through the root boundary', async () => {
  const RootComponent = React.forwardRef<{ value: number }, { value: number }>(
    (props, ref) => {
      React.useImperativeHandle(ref, () => ({ value: props.value }), [
        props.value,
      ]);
      return <button>{props.value}</button>;
    },
  );
  const Root = withTelemetryRoot(RootComponent);
  const ref = React.createRef<{ value: number }>();
  SessionTelemetry.install();
  await act(() => {
    renderer = TestRenderer.create(<Root ref={ref} value={42} />);
  });
  expect(ref.current).toEqual({ value: 42 });
  await act(() => renderer!.update(<Root ref={ref} value={43} />));
  expect(ref.current).toEqual({ value: 43 });
});

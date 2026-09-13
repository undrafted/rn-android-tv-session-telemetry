import React, {
  Profiler,
  useEffect,
  useLayoutEffect,
  useRef,
  useState,
} from 'react';
import { Text, View } from 'react-native';
import { onProfilerRender } from '@rn-session-telemetry/react-native';
import { runOverheadBenchmark } from './benchmark';

function Workload({ tick, complete }: { tick: number; complete: () => void }) {
  useLayoutEffect(complete, [complete, tick]);
  return (
    <View>
      {Array.from({ length: 40 }, (_, i) => (
        <Text key={i}>{tick + i}</Text>
      ))}
    </View>
  );
}

export default function BenchmarkApp() {
  const [task, setTask] = useState<{
    tick: number;
    profiled: boolean;
    complete: () => void;
  }>();
  const [status, setStatus] = useState('Benchmark running…');
  const started = useRef(false);
  useEffect(() => {
    if (started.current) return;
    started.current = true;
    let tick = 0;
    runOverheadBenchmark({
      render: profiled =>
        new Promise<void>((resolve, reject) => {
          const timeout = setTimeout(
            () => reject(new Error('Benchmark render timed out')),
            5000,
          );
          setTask({
            tick: ++tick,
            profiled,
            complete: () => {
              clearTimeout(timeout);
              resolve();
            },
          });
        }),
    })
      .then(result => {
        for (const [name, eventType, count] of [
          ['network-fetch', 'network', result.config.requests],
          ['react-render', 'react-commit', result.config.renders],
        ] as const) {
          const entry = result.results.find(row => row.name === name)!;
          if (
            entry.samples.some(
              sample =>
                sample.active && (sample.eventCounts[eventType] ?? 0) !== count,
            )
          )
            throw new Error(
              `Missing or duplicate ${eventType} capture in benchmark`,
            );
        }
        // eslint-disable-next-line no-console
        console.log(
          'RNST_BENCHMARK_CONTEXT',
          JSON.stringify({
            config: result.config,
            device: result.device,
            limits: result.limits,
          }),
        );
        for (const entry of result.results) {
          // eslint-disable-next-line no-console
          console.log('RNST_BENCHMARK_CASE', JSON.stringify(entry));
        }
        // eslint-disable-next-line no-console
        console.log('RNST_OVERHEAD_BENCHMARK_COMPLETE');
        setStatus('Benchmark complete. Results in logcat.');
      })
      .catch(error => {
        // eslint-disable-next-line no-console
        console.error('RNST_OVERHEAD_BENCHMARK_ERROR', String(error));
        setStatus(String(error));
      });
  }, []);
  const content = task && (
    <Workload tick={task.tick} complete={task.complete} />
  );
  return (
    <View>
      <Text>{status}</Text>
      {task?.profiled ? (
        <Profiler id="benchmark-root" onRender={onProfilerRender}>
          {content}
        </Profiler>
      ) : (
        content
      )}
    </View>
  );
}

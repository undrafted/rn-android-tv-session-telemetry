import { createElement, Profiler, type ComponentType, type ProfilerOnRenderCallback } from 'react';
import { SessionTelemetry } from './index.js';

// React reports render work for a committed subtree, not time spent in the commit phase.
// Keep the legacy callback available; telemetry failure must never interrupt a React commit.
export const onProfilerRender: ProfilerOnRenderCallback = (
  id,
  phase,
  actualDuration,
  baseDuration,
  startTime,
  commitTime,
) => {
  try {
    SessionTelemetry.recordReactCommit(
      id,
      phase,
      actualDuration,
      baseDuration,
      startTime,
      commitTime,
    );
  } catch {
    // Recording is observational, including when the optional native writer fails.
  }
};

// Call once at root registration in a profiling build, after install(). The root's identity
// and Profiler remain stable across stop/reinstall so component state and effects survive.
// Production renderers simply omit callbacks; selecting a profiling renderer is a build task.
export function withTelemetryRoot<Props extends object>(
  Root: ComponentType<Props>,
  profilerId = 'App',
): ComponentType<Props> {
  function TelemetryRoot(props: Props) {
    return createElement(
      Profiler,
      { id: profilerId, onRender: onProfilerRender },
      createElement(Root, props),
    );
  }
  return TelemetryRoot;
}

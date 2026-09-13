import type { ProfilerOnRenderCallback } from 'react';
import { SessionTelemetry } from './index.js';

// Pass this directly as a <Profiler onRender={onProfilerRender}> callback. React invokes it
// synchronously after every commit of the profiled subtree, so this stays a thin adapter onto
// SessionTelemetry.recordReactCommit rather than a component of its own — matching the "minimal
// turn on/off mechanism" goal (no extra JSX wrapper beyond the one <Profiler> the app already
// author). id/phase/actualDuration/baseDuration are React's own measurements, passed through
// unchanged (plan.md section 14 #2: prefer the public Profiler hook over internals).
export const onProfilerRender: ProfilerOnRenderCallback = (id, phase, actualDuration, baseDuration) => {
  SessionTelemetry.recordReactCommit(id, phase, actualDuration, baseDuration);
};

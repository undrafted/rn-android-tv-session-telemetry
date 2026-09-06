import type { Middleware, UnknownAction } from 'redux';
import { SessionTelemetry } from '@rn-session-telemetry/react-native';

export interface TelemetryMiddlewareOptions {
  includeActionTypes: boolean;
  includePayloads: false;
}

function isKnownAction(action: unknown): action is UnknownAction {
  return typeof action === 'object' && action !== null && 'type' in action;
}

export function createTelemetryMiddleware(options: TelemetryMiddlewareOptions): Middleware {
  return () => (next) => (action) => {
    if (!isKnownAction(action)) {
      return next(action);
    }

    // Dispatch duration measured with the same wall-clock source SessionTelemetry uses
    // elsewhere; recordDispatch() itself is a real no-op until install() runs. try/finally so
    // a throwing reducer/downstream middleware still gets recorded, not silently dropped.
    const start = performance.now();
    try {
      return next(action);
    } finally {
      const durationMs = performance.now() - start;
      SessionTelemetry.recordDispatch(
        options.includeActionTypes ? action.type : 'redux/action',
        durationMs,
      );
    }
  };
}

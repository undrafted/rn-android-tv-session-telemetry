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

// A transparent instrumentation wrapper around a selector - calls `selector` exactly once per
// invocation and returns its exact result unchanged, so it can never affect the wrapped
// selector's own memoization semantics (reselect's or hand-rolled). Compares this call's
// arguments and result against the previous call's by reference only (Object.is), never their
// contents - metadata only, same discipline as createTelemetryMiddleware's
// includePayloads: false. The very first call has nothing to compare against, so both flags
// read true (everything is "new"), which keeps invocation counts accurate rather than silently
// skipping the first call's event.
export function telemetrySelector<F extends (...args: never[]) => unknown>(
  selectorId: string,
  selector: F,
): F {
  let previousArgs: unknown[] | undefined;
  let previousResult: unknown;

  return ((...args: unknown[]) => {
    const start = performance.now();
    const result = selector(...(args as never[]));
    const durationMs = performance.now() - start;

    const inputsChanged =
      previousArgs === undefined ||
      previousArgs.length !== args.length ||
      previousArgs.some((previousArg, index) => !Object.is(previousArg, args[index]));
    const resultChanged = previousResult === undefined || !Object.is(previousResult, result);

    SessionTelemetry.recordSelector(selectorId, durationMs, inputsChanged, resultChanged);

    previousArgs = args;
    previousResult = result;
    return result;
  }) as unknown as F;
}

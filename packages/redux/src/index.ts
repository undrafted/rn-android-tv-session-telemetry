import type { Middleware } from 'redux';

export interface TelemetryMiddlewareOptions {
  includeActionTypes: boolean;
  includePayloads: false;
}

export function createTelemetryMiddleware(_options: TelemetryMiddlewareOptions): Middleware {
  return () => (next) => (action) => next(action);
}

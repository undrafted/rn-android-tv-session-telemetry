export interface SessionTelemetryApi {
  install(): void;
  stop(): void;
  mark(name: string): void;
}

const noop: SessionTelemetryApi = {
  install() {},
  stop() {},
  mark(_name: string) {},
};

export const SessionTelemetry: SessionTelemetryApi = noop;

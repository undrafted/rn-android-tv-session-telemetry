import { Pressable, type PressableProps } from 'react-native';
import { SessionTelemetry } from './index.js';

// INTERIM ONLY. The project's goal is minimal instrumentation footprint — not asking the host
// app to change many components. Wrapping every Pressable and assigning it an id is the
// opposite of that, so this exists only until a native global focus listener
// (ViewTreeObserver.addOnGlobalFocusChangeListener on Android) replaces it with something that
// needs zero per-component changes. Prefer that once it exists; don't build more on top of
// this pattern in the meantime.
export interface FocusableViewProps extends PressableProps {
  // Stable identifier recorded as the focus event's target. Not a native view tag.
  id: string;
}

export function FocusableView({ id, onFocus, ...rest }: FocusableViewProps) {
  return (
    <Pressable
      {...rest}
      onFocus={(event) => {
        SessionTelemetry.recordFocus(id);
        onFocus?.(event);
      }}
    />
  );
}

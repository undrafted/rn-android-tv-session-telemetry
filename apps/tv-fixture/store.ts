/**
 * A minimal Redux store demonstrating M7's selector instrumentation — one stable selector and
 * one deliberately unstable one, both wrapped with telemetrySelector so a normal interaction in
 * this app produces the real "repeated recomputation" and "unstable reference" findings, not a
 * contrived one.
 *
 * @format
 */

import { useSyncExternalStore } from 'react';
import { applyMiddleware, createStore } from 'redux';
import {
  createTelemetryMiddleware,
  telemetrySelector,
} from '@rn-session-telemetry/redux';

interface CatalogItem {
  id: string;
}

export interface AppState {
  catalog: {
    items: CatalogItem[];
    focusedId: string | null;
  };
}

type AppAction = { type: 'catalog/cardFocused'; targetId: string };

const initialState: AppState = {
  catalog: {
    items: [{ id: 'card-1' }, { id: 'card-2' }],
    focusedId: null,
  },
};

// `items` is never replaced by this reducer — only `focusedId` changes — so any selector that
// only reads `state.catalog.items` has a genuinely unchanged input across a focus change; the
// bug demonstrated below is that .map() doesn't know that and returns a new array anyway.
function reducer(state: AppState = initialState, action: AppAction): AppState {
  if (action.type === 'catalog/cardFocused') {
    return {
      ...state,
      catalog: { ...state.catalog, focusedId: action.targetId },
    };
  }
  return state;
}

export const store = createStore(
  reducer,
  applyMiddleware(
    createTelemetryMiddleware({
      includeActionTypes: true,
      includePayloads: false,
    }),
  ),
);

// Good: a primitive result is trivially stable across calls (Object.is(2, 2) is true) even
// though it's recomputed from scratch every time.
export const selectItemCount = telemetrySelector(
  'catalog/selectItemCount',
  (state: AppState) => state.catalog.items.length,
);

// Bad: .map() returns a fresh array every call even though state.catalog.items itself never
// changes reference — the classic unstable-selector bug this milestone's detectors catch.
export const selectVisibleItemIds = telemetrySelector(
  'catalog/selectVisibleItemIds',
  (state: AppState) => state.catalog.items.map(item => item.id),
);

// Only safe with a selector whose result is reference-stable when nothing relevant changed
// (e.g. selectItemCount) — useSyncExternalStore requires its snapshot to satisfy that, or React
// treats every render as "the store changed again" and loops. An unstable selector like
// selectVisibleItemIds must be called directly during render instead (see App.tsx), not through
// this hook.
export function useAppSelector<T>(selector: (state: AppState) => T): T {
  return useSyncExternalStore(store.subscribe, () =>
    selector(store.getState()),
  );
}

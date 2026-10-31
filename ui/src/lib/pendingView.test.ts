import { describe, expect, it, vi } from 'vitest';
import type { View } from './bindings';
import { createMockBackend } from './mock/mockBackend';
import { applyPendingView, asView } from './pendingView';

const tauri = (take: () => Promise<string | null>) => ({
  kind: 'tauri' as const,
  takePendingView: vi.fn(take),
});

describe('asView', () => {
  it('only lets the two known views through', () => {
    expect(asView('editor')).toBe('editor');
    expect(asView('settings')).toBe('settings');
    for (const bad of ['', 'Editor', 'about', null, undefined, 3, {}]) {
      expect(asView(bad)).toBeNull();
    }
  });
});

describe('applyPendingView', () => {
  it('opens the parked view exactly once', async () => {
    const seen: View[] = [];
    const backend = tauri(async () => 'settings');
    expect(await applyPendingView(backend, (v) => seen.push(v))).toBe('settings');
    expect(seen).toEqual(['settings']);
    expect(backend.takePendingView).toHaveBeenCalledTimes(1);
  });

  it('does nothing when no view is parked', async () => {
    const navigate = vi.fn();
    expect(await applyPendingView(tauri(async () => null), navigate)).toBeNull();
    expect(navigate).not.toHaveBeenCalled();
  });

  it('ignores a view it does not know', async () => {
    const navigate = vi.fn();
    expect(await applyPendingView(tauri(async () => 'about'), navigate)).toBeNull();
    expect(navigate).not.toHaveBeenCalled();
  });

  it('survives a backend without the command or with a failing one', async () => {
    const navigate = vi.fn();
    const missing = tauri(() => Promise.reject('command take_pending_view not found'));
    expect(await applyPendingView(missing, navigate)).toBeNull();
    const throwing = tauri(() => {
      throw new Error('boom');
    });
    expect(await applyPendingView(throwing, navigate)).toBeNull();
    expect(navigate).not.toHaveBeenCalled();
  });

  it('never asks the browser demo (mock) at all', async () => {
    const navigate = vi.fn();
    const mock = createMockBackend({ persist: false });
    const spy = vi.spyOn(mock, 'takePendingView');
    expect(await applyPendingView(mock, navigate)).toBeNull();
    expect(spy).not.toHaveBeenCalled();
    expect(navigate).not.toHaveBeenCalled();
  });

  it('the mock backend has nothing parked', async () => {
    expect(await createMockBackend({ persist: false }).takePendingView()).toBeNull();
  });
});

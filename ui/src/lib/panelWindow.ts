// The quick panel's own window: place, show, hide, and hand off to the main window.
// Only the panel entry imports this, so the main bundle never pulls in the Tauri
// window API. In demo mode every call is recorded on `panelDemo` for tests.

import { invoke } from '@tauri-apps/api/core';
import { emitTo } from '@tauri-apps/api/event';
import { getCurrentWindow, LogicalSize, PhysicalPosition } from '@tauri-apps/api/window';
import { api } from './backend';
import type { Placement } from './panelPlacement';

/** What the panel asked the main window to show. */
export type PanelTarget = 'switcher' | 'cheat_sheet' | 'settings' | 'corners';

export interface PanelDemoLog {
  placed: Placement | null;
  hidden: number;
  opened: PanelTarget[];
}

/** Demo-mode record of window calls (null in the real app). */
export const panelDemo: PanelDemoLog | null = api.kind === 'mock' ? { placed: null, hidden: 0, opened: [] } : null;

export async function placeWindow(p: Placement): Promise<void> {
  if (panelDemo) {
    panelDemo.placed = p;
    return;
  }
  const w = getCurrentWindow();
  await w.setSize(new LogicalSize(p.width, p.height));
  await w.setPosition(new PhysicalPosition(p.x, p.y));
  await w.show();
  await w.setFocus();
}

export async function hideWindow(): Promise<void> {
  if (panelDemo) {
    panelDemo.hidden++;
    return;
  }
  await getCurrentWindow().hide();
}

/**
 * Show the main window at `target`. The backend's `open_main` command creates or
 * focuses it and sends it a "navigate" event; an older backend without the command
 * gets the event straight from here (works while the main window exists).
 */
export async function openMain(target: PanelTarget): Promise<void> {
  if (panelDemo) {
    panelDemo.opened.push(target);
    return;
  }
  try {
    await invoke('open_main', { view: target });
  } catch {
    await emitTo('main', 'navigate', target).catch(() => undefined);
  }
}

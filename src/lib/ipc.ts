/** Tauri invoke 封装（命令名与参数与 src-tauri/src/commands.rs 一一对应）。 */

import { invoke } from '@tauri-apps/api/core';

import type { FrozenPayload, PhysRect, Settings } from './types';

export const fetchSession = () => invoke<FrozenPayload | null>('fetch_session');

export const revealOverlay = () => invoke('reveal_overlay');

export const endSession = (outcome: 'confirmed' | 'cancelled') =>
  invoke('end_session', { outcome });

export const triggerCapture = () => invoke('trigger_capture');

export const getSettings = () => invoke<Settings>('get_settings');

export const setHotkey = (combo: string) => invoke<string>('set_hotkey', { combo });

export const setAutostart = (enabled: boolean) => invoke('set_autostart', { enabled });

export const copyImage = (selection: PhysRect, annotationsPng: Uint8Array) =>
  invoke('copy_image', { selection, annotationsPng });

export const saveImage = (
  selection: PhysRect,
  annotationsPng: Uint8Array,
  path: string,
  format: string,
) => invoke('save_image', { selection, annotationsPng, path, format });

export const logMetric = (name: string, ms: number) => invoke('log_metric', { name, ms });

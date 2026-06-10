// Tauri bindings for the dashboard's pull adapters (§7/§6.2 read seams).
//
// The Halyard CLI-spawn read and the Audience HTTP poll both cross a process / origin
// boundary the browser can't (CLI spawn; devAuth + CORS), so they run in Rust
// (`src-tauri/src/dashboard.rs`) and are invoked here. These wrappers implement the
// `HalyardReader` / `AudienceReader` seams so the adapters stay environment-agnostic
// and unit-testable with fakes.

import { invoke } from '@tauri-apps/api/core';
import type { HalyardReader, HalyardReleaseStatus, HalyardProposal } from './adapters/halyard';
import type { AudienceReader, AudiencePost } from './adapters/audience';

/** Reads Halyard by spawning the `halyard` CLI (Rust side parses stdout JSON). */
export const tauriHalyardReader: HalyardReader = {
  status: () => invoke<HalyardReleaseStatus[]>('halyard_status'),
  queue: () => invoke<HalyardProposal[]>('halyard_queue'),
};

/** Reads Audience over HTTP from the Rust side (devAuth/CORS-free). */
export const tauriAudienceReader: AudienceReader = {
  health: () => invoke<boolean>('audience_health'),
  posts: () => invoke<AudiencePost[]>('audience_posts'),
};

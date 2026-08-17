import { Injectable } from '@angular/core';
import { invoke } from '@tauri-apps/api/core';

export interface ProcessInfo {
  pid: number;
  name: string;
}

export interface CheatMeta {
  id: string;
  name: string;
  description: string;
  hotkey: string;
}

export type CheatState = 'not_applied' | 'applied' | 'unsupported';

export interface CheatStatus {
  cheat_id: string;
  state: CheatState;
  version_label: string | null;
}

export interface InstantBuildStatus {
  enabled: boolean;
}

@Injectable({ providedIn: 'root' })
export class TrainerService {
  detectProcess(): Promise<ProcessInfo | null> {
    return invoke('detect_process');
  }

  resolvePid(pid: number): Promise<ProcessInfo | null> {
    return invoke('resolve_pid', { pid });
  }

  getCheats(): Promise<CheatMeta[]> {
    return invoke('get_cheats');
  }

  refreshStatus(pid: number): Promise<CheatStatus[]> {
    return invoke('refresh_status', { pid });
  }

  applyCheat(pid: number, cheatId: string): Promise<CheatStatus> {
    return invoke('apply_cheat', { pid, cheatId });
  }

  toggleInstantBuild(pid: number, enabled: boolean): Promise<InstantBuildStatus> {
    return invoke('toggle_instant_build', { pid, enabled });
  }

  // Keeps the backend's notion of "which process are we acting on" in sync
  // so global-shortcut presses (which don't go through the frontend) know
  // which pid to target.
  setActivePid(pid: number | null): Promise<void> {
    return invoke('set_active_pid', { pid });
  }

  instantBuildHotkey(): Promise<string> {
    return invoke('instant_build_hotkey');
  }
}

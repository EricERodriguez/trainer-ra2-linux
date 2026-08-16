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
}

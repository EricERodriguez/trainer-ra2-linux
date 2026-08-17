import { Component, OnInit, computed, inject, signal } from '@angular/core';
import { FormsModule } from '@angular/forms';
import { listen } from '@tauri-apps/api/event';
import { CheatListComponent, CheatRow } from './cheat-list';
import { CheatMeta, CheatStatus, InstantBuildStatus, ProcessInfo, TrainerService } from './trainer.service';

@Component({
  selector: 'app-root',
  imports: [FormsModule, CheatListComponent],
  templateUrl: './app.html',
  styleUrl: './app.css',
})
export class App implements OnInit {
  private readonly trainer = inject(TrainerService);

  process = signal<ProcessInfo | null>(null);
  manualPidInput = '';
  cheatMeta = signal<CheatMeta[]>([]);
  statuses = signal<Map<string, CheatStatus>>(new Map());
  detecting = signal(false);
  refreshing = signal(false);
  togglingId = signal<string | null>(null);
  errorMsg = signal<string | null>(null);
  instantBuildEnabled = signal(false);
  instantBuildBusy = signal(false);
  instantBuildHotkey = signal('');

  cheatRows = computed<CheatRow[]>(() =>
    this.cheatMeta().map((meta) => {
      const status = this.statuses().get(meta.id);
      return {
        id: meta.id,
        name: meta.name,
        description: meta.description,
        hotkey: meta.hotkey,
        state: status?.state ?? 'unknown',
        versionLabel: status?.version_label ?? null,
      };
    }),
  );

  async ngOnInit() {
    try {
      this.cheatMeta.set(await this.trainer.getCheats());
      this.instantBuildHotkey.set(await this.trainer.instantBuildHotkey());
    } catch (e) {
      this.errorMsg.set(String(e));
    }

    // Global shortcuts fire in the Rust backend (so they work even while
    // the game window has focus, not this app's window) and report back
    // through these events instead of a command's return value.
    await listen<CheatStatus>('cheat-status-changed', (event) => {
      const next = new Map(this.statuses());
      next.set(event.payload.cheat_id, event.payload);
      this.statuses.set(next);
    });
    await listen<InstantBuildStatus>('instant-build-changed', (event) => {
      this.instantBuildEnabled.set(event.payload.enabled);
    });
    await listen<string>('hotkey-error', (event) => {
      this.errorMsg.set(event.payload);
    });

    await this.detectProcess();
  }

  async detectProcess() {
    this.detecting.set(true);
    this.errorMsg.set(null);
    try {
      const proc = await this.trainer.detectProcess();
      this.process.set(proc);
      this.instantBuildEnabled.set(false);
      await this.trainer.setActivePid(proc ? proc.pid : null);
      if (proc) {
        await this.refreshStatus();
      }
    } catch (e) {
      this.errorMsg.set(String(e));
    } finally {
      this.detecting.set(false);
    }
  }

  async useManualPid() {
    const pid = Number(this.manualPidInput);
    if (!Number.isInteger(pid) || pid <= 0) {
      this.errorMsg.set('Ingresá un PID numérico válido');
      return;
    }
    this.detecting.set(true);
    this.errorMsg.set(null);
    try {
      const proc = await this.trainer.resolvePid(pid);
      if (!proc) {
        this.errorMsg.set(`No existe ningún proceso con PID ${pid}`);
        this.process.set(null);
        return;
      }
      this.process.set(proc);
      this.instantBuildEnabled.set(false);
      await this.trainer.setActivePid(proc.pid);
      await this.refreshStatus();
    } catch (e) {
      this.errorMsg.set(String(e));
    } finally {
      this.detecting.set(false);
    }
  }

  async refreshStatus() {
    const proc = this.process();
    if (!proc) return;
    this.refreshing.set(true);
    this.errorMsg.set(null);
    try {
      const statuses = await this.trainer.refreshStatus(proc.pid);
      this.statuses.set(new Map(statuses.map((s) => [s.cheat_id, s])));
    } catch (e) {
      this.errorMsg.set(String(e));
    } finally {
      this.refreshing.set(false);
    }
  }

  async toggleCheat(cheatId: string) {
    const proc = this.process();
    if (!proc) return;
    this.togglingId.set(cheatId);
    this.errorMsg.set(null);
    try {
      const status = await this.trainer.toggleCheat(proc.pid, cheatId);
      const next = new Map(this.statuses());
      next.set(status.cheat_id, status);
      this.statuses.set(next);
    } catch (e) {
      this.errorMsg.set(String(e));
    } finally {
      this.togglingId.set(null);
    }
  }

  async toggleInstantBuild() {
    const proc = this.process();
    if (!proc) return;
    const next = !this.instantBuildEnabled();
    this.instantBuildBusy.set(true);
    this.errorMsg.set(null);
    try {
      const status = await this.trainer.toggleInstantBuild(proc.pid, next);
      this.instantBuildEnabled.set(status.enabled);
    } catch (e) {
      this.errorMsg.set(String(e));
    } finally {
      this.instantBuildBusy.set(false);
    }
  }
}

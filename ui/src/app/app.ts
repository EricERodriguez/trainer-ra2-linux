import { Component, OnInit, computed, inject, signal } from '@angular/core';
import { FormsModule } from '@angular/forms';
import { CheatListComponent, CheatRow } from './cheat-list';
import { CheatMeta, CheatStatus, ProcessInfo, TrainerService } from './trainer.service';

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
  applyingId = signal<string | null>(null);
  errorMsg = signal<string | null>(null);

  cheatRows = computed<CheatRow[]>(() =>
    this.cheatMeta().map((meta) => {
      const status = this.statuses().get(meta.id);
      return {
        id: meta.id,
        name: meta.name,
        description: meta.description,
        state: status?.state ?? 'unknown',
        versionLabel: status?.version_label ?? null,
      };
    }),
  );

  async ngOnInit() {
    try {
      this.cheatMeta.set(await this.trainer.getCheats());
    } catch (e) {
      this.errorMsg.set(String(e));
    }
    await this.detectProcess();
  }

  async detectProcess() {
    this.detecting.set(true);
    this.errorMsg.set(null);
    try {
      const proc = await this.trainer.detectProcess();
      this.process.set(proc);
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

  async applyCheat(cheatId: string) {
    const proc = this.process();
    if (!proc) return;
    this.applyingId.set(cheatId);
    this.errorMsg.set(null);
    try {
      const status = await this.trainer.applyCheat(proc.pid, cheatId);
      const next = new Map(this.statuses());
      next.set(status.cheat_id, status);
      this.statuses.set(next);
    } catch (e) {
      this.errorMsg.set(String(e));
    } finally {
      this.applyingId.set(null);
    }
  }
}

import { Component, input, output } from '@angular/core';
import type { CheatState } from './trainer.service';

export interface CheatRow {
  id: string;
  name: string;
  description: string;
  hotkey: string;
  state: CheatState | 'unknown';
  versionLabel: string | null;
}

@Component({
  selector: 'app-cheat-list',
  imports: [],
  templateUrl: './cheat-list.html',
  styleUrl: './cheat-list.css',
})
export class CheatListComponent {
  cheats = input.required<CheatRow[]>();
  togglingId = input<string | null>(null);
  canToggle = input(false);
  toggle = output<string>();

  canToggleRow(cheat: CheatRow): boolean {
    return this.canToggle() && (cheat.state === 'not_applied' || cheat.state === 'applied');
  }

  buttonLabel(cheat: CheatRow): string {
    const busy = this.togglingId() === cheat.id;
    if (cheat.state === 'applied') return busy ? 'Quitando…' : 'Quitar';
    return busy ? 'Aplicando…' : 'Aplicar';
  }
}

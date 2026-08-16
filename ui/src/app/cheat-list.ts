import { Component, input, output } from '@angular/core';
import type { CheatState } from './trainer.service';

export interface CheatRow {
  id: string;
  name: string;
  description: string;
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
  applyingId = input<string | null>(null);
  canApply = input(false);
  apply = output<string>();
}

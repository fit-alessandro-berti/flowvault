import { Component, input } from '@angular/core';

@Component({
  standalone: true,
  selector: 'app-pattern-explorer',
  templateUrl: './pattern-explorer.component.html',
})
export class PatternExplorerComponent {
  readonly vmInput = input.required<any>({ alias: 'vm' });

  protected get vm(): any {
    return this.vmInput();
  }
}

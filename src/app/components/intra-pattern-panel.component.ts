import { Component, input } from '@angular/core';

@Component({
  standalone: true,
  selector: 'app-intra-pattern-panel',
  templateUrl: './intra-pattern-panel.component.html',
})
export class IntraPatternPanelComponent {
  readonly vmInput = input.required<any>({ alias: 'vm' });

  protected get vm(): any {
    return this.vmInput();
  }
}

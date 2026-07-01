import { Component, input } from '@angular/core';

@Component({
  standalone: true,
  selector: 'app-inter-pattern-panel',
  templateUrl: './inter-pattern-panel.component.html',
})
export class InterPatternPanelComponent {
  readonly vmInput = input.required<any>({ alias: 'vm' });

  protected get vm(): any {
    return this.vmInput();
  }
}

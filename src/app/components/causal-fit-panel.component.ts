import { Component, input } from '@angular/core';

@Component({
  standalone: true,
  selector: 'app-causal-fit-panel',
  templateUrl: './causal-fit-panel.component.html',
})
export class CausalFitPanelComponent {
  readonly vmInput = input.required<any>({ alias: 'vm' });

  protected get vm(): any {
    return this.vmInput();
  }
}

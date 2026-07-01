import { Component, input } from '@angular/core';

@Component({
  standalone: true,
  selector: 'app-causal-edge-panel',
  templateUrl: './causal-edge-panel.component.html',
})
export class CausalEdgePanelComponent {
  readonly vmInput = input.required<any>({ alias: 'vm' });

  protected get vm(): any {
    return this.vmInput();
  }
}

import { Component, input } from '@angular/core';

@Component({
  standalone: true,
  selector: 'app-causal-workbench',
  templateUrl: './causal-workbench.component.html',
})
export class CausalWorkbenchComponent {
  readonly vmInput = input.required<any>({ alias: 'vm' });

  protected get vm(): any {
    return this.vmInput();
  }
}

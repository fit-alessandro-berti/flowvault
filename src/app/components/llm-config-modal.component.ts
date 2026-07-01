import { Component, input } from '@angular/core';

@Component({
  standalone: true,
  selector: 'app-llm-config-modal',
  templateUrl: './llm-config-modal.component.html',
})
export class LlmConfigModalComponent {
  readonly vmInput = input.required<any>({ alias: 'vm' });

  protected get vm(): any {
    return this.vmInput();
  }
}

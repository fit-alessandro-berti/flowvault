import { Component, input } from '@angular/core';

@Component({
  standalone: true,
  selector: 'app-state-detection-page',
  templateUrl: './state-detection-page.component.html',
})
export class StateDetectionPageComponent {
  readonly vmInput = input.required<any>({ alias: 'vm' });

  protected get vm(): any {
    return this.vmInput();
  }
}

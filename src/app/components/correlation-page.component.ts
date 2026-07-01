import { Component, input } from '@angular/core';

@Component({
  standalone: true,
  selector: 'app-correlation-page',
  templateUrl: './correlation-page.component.html',
})
export class CorrelationPageComponent {
  readonly vmInput = input.required<any>({ alias: 'vm' });

  protected get vm(): any {
    return this.vmInput();
  }
}

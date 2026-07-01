import { Component, input } from '@angular/core';

@Component({
  standalone: true,
  selector: 'app-time-perspective-page',
  templateUrl: './time-perspective-page.component.html',
})
export class TimePerspectivePageComponent {
  readonly vmInput = input.required<any>({ alias: 'vm' });

  protected get vm(): any {
    return this.vmInput();
  }
}

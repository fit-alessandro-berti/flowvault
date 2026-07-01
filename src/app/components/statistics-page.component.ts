import { Component, input } from '@angular/core';

@Component({
  standalone: true,
  selector: 'app-statistics-page',
  templateUrl: './statistics-page.component.html',
})
export class StatisticsPageComponent {
  readonly vmInput = input.required<any>({ alias: 'vm' });

  protected get vm(): any {
    return this.vmInput();
  }
}

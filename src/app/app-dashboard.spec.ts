import { TestBed } from '@angular/core/testing';
import { vi } from 'vitest';
import { App } from './app';
import type { ProcessGraph } from './ocel-wasm.service';
import { importedSummary, patternAnalysis, processGraph, statefulSummary, traditionalProcessGraph } from './testing/core-fixtures';
import { lifecycleDetail, objectSearchResult, stateCorrelationAnalysis, transitionKpisAnalysis } from './testing/dashboard-fixtures';
import { stateDetectionAnalysis, stateDetectionCellDetail } from './testing/state-detection-fixtures';

describe('App', () => {
  beforeEach(async () => {
    localStorage.clear();
    await TestBed.configureTestingModule({
      imports: [App],
    }).compileComponents();
  });

  it('renders feature correlations for the currently applied state', () => {
    const fixture = TestBed.createComponent(App);
    const component = fixture.componentInstance as unknown as {
      documentHandle: unknown;
      fileName: { set(value: string): void };
      summary: { set(value: unknown): void };
      originalSummary: { set(value: unknown): void };
      filterOptions: { set(value: unknown): void };
      selectedObjectTypes: { set(value: string[]): void };
      selectedEventTypes: { set(value: string[]): void };
    };
    let correlationRequests = 0;

    component.documentHandle = {
      stateCorrelationsJson: () => {
        correlationRequests += 1;
        return JSON.stringify(stateCorrelationAnalysis);
      },
    };
    component.fileName.set('orders.json');
    component.summary.set(statefulSummary);
    component.originalSummary.set(statefulSummary);
    component.filterOptions.set({
      event_types: ['Create Order', 'Close Order'],
      object_types: ['Order', 'Item'],
      text_attributes: [{ scope: 'event', name: 'state', values: ['Open', 'Closed'] }],
    });
    component.selectedEventTypes.set(['Create Order', 'Close Order']);
    component.selectedObjectTypes.set(['Order', 'Item']);
    fixture.detectChanges();

    const native = fixture.nativeElement as HTMLElement;
    Array.from(native.querySelectorAll<HTMLButtonElement>('.feature-button'))
      .find((button) => button.textContent?.includes('Correlation'))
      ?.click();
    fixture.detectChanges();

    expect(correlationRequests).toBe(1);
    expect(native.textContent).toContain('Correlation');
    expect(native.textContent).toContain('Feature-State Correlation');
    expect(native.textContent).toContain('activity.Create Order');
    expect(native.textContent).toContain('activity.Close Order');
    expect(native.textContent).toContain('+0.820');
    expect(native.textContent).toContain('-0.640');
    expect(native.textContent).toContain('82%');
    expect(native.textContent).toContain('5 / 8');
    expect(native.querySelector('.correlation-table-scroll')).toBeTruthy();
    expect(
      native.querySelector<HTMLElement>('.correlation-value')?.getAttribute('style'),
    ).toContain('hsl');
    expect(native.querySelector('.correlation-table-panel header span')?.getAttribute('title')).toBe(
      'Open: 5, Closed: 3',
    );

    native.querySelector<HTMLButtonElement>('.correlation-header .ghost-button')?.click();
    fixture.detectChanges();
    expect(correlationRequests).toBe(2);
  });

  it('renders transition KPIs and object lifecycle timelines', () => {
    const fixture = TestBed.createComponent(App);
    const component = fixture.componentInstance as unknown as {
      documentHandle: unknown;
      fileName: { set(value: string): void };
      summary: { set(value: unknown): void };
      originalSummary: { set(value: unknown): void };
      filterOptions: { set(value: unknown): void };
      selectedObjectTypes: { set(value: string[]): void };
      selectedEventTypes: { set(value: string[]): void };
    };
    let kpiRequests = 0;
    let searchRequests = 0;
    let lifecycleRequests = 0;

    component.documentHandle = {
      stateTransitionKpisJson: () => {
        kpiRequests += 1;
        return JSON.stringify(transitionKpisAnalysis);
      },
      objectSearchJson: () => {
        searchRequests += 1;
        return JSON.stringify(objectSearchResult);
      },
      objectLifecycleDetailJson: (objectId: string) => {
        lifecycleRequests += 1;
        expect(objectId).toBe('O1');
        return JSON.stringify(lifecycleDetail);
      },
    };
    component.fileName.set('orders.json');
    component.summary.set(statefulSummary);
    component.originalSummary.set(statefulSummary);
    component.filterOptions.set({
      event_types: ['Create Order', 'Close Order'],
      object_types: ['Order', 'Item'],
      text_attributes: [{ scope: 'event', name: 'state', values: ['Open', 'Closed'] }],
    });
    component.selectedEventTypes.set(['Create Order', 'Close Order']);
    component.selectedObjectTypes.set(['Order', 'Item']);
    fixture.detectChanges();

    const native = fixture.nativeElement as HTMLElement;
    Array.from(native.querySelectorAll<HTMLButtonElement>('.feature-button'))
      .find((button) => button.textContent?.includes('Transition KPIs'))
      ?.click();
    fixture.detectChanges();

    expect(kpiRequests).toBe(1);
    expect(native.textContent).toContain('Transition Matrix');
    expect(native.textContent).toContain('Open -> Closed');
    expect(native.textContent).toContain('Stuck In State');
    expect(native.textContent).toContain('O1');

    Array.from(native.querySelectorAll<HTMLButtonElement>('.feature-button'))
      .find((button) => button.textContent?.includes('Lifecycle Timeline'))
      ?.click();
    fixture.detectChanges();

    expect(searchRequests).toBe(1);
    expect(lifecycleRequests).toBe(1);
    expect(native.textContent).toContain('Lifecycle Timeline');
    expect(native.textContent).toContain('Create Order');
    expect(native.textContent).toContain('Stock After');
    expect(native.textContent).toContain('Related Objects');
    expect(native.querySelector('.lifecycle-timeline')).toBeTruthy();
  });
});

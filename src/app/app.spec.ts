import { TestBed } from '@angular/core/testing';
import { App } from './app';

describe('App', () => {
  beforeEach(async () => {
    await TestBed.configureTestingModule({
      imports: [App],
    }).compileComponents();
  });

  it('creates the inspector shell', () => {
    const fixture = TestBed.createComponent(App);
    fixture.detectChanges();

    expect(fixture.componentInstance).toBeTruthy();
    expect((fixture.nativeElement as HTMLElement).querySelector('h1')?.textContent).toContain(
      'OCEL 2.0 Inspector',
    );
  });

  it('keeps export buttons disabled before import', () => {
    const fixture = TestBed.createComponent(App);
    fixture.detectChanges();

    const buttons = Array.from((fixture.nativeElement as HTMLElement).querySelectorAll('button'));
    expect(buttons.length).toBe(3);
    expect(buttons.every((button) => button.disabled)).toBe(true);
  });

  it('renders empty summary counts initially', () => {
    const fixture = TestBed.createComponent(App);
    fixture.detectChanges();

    const values = Array.from(
      (fixture.nativeElement as HTMLElement).querySelectorAll('.summary-card strong'),
    ).map((element) => element.textContent?.trim());

    expect(values).toEqual(['0', '0', '0', '0']);
  });

  it('opens state preset dialog after import', () => {
    const fixture = TestBed.createComponent(App);
    const component = fixture.componentInstance as unknown as {
      documentHandle: unknown;
      fileName: { set(value: string): void };
      summary: { set(value: unknown): void };
      openStateDialog(): void;
    };

    component.documentHandle = {};
    component.fileName.set('order-management.json');
    component.summary.set({
      source_format: 'json',
      event_types: 1,
      object_types: 1,
      events: 1,
      objects: 1,
      e2o_relationships: 1,
      o2o_relationships: 0,
      interned_strings: 1,
      objects_with_lifecycle: 1,
      stateful_events: 0,
    });
    component.openStateDialog();
    fixture.detectChanges();

    const native = fixture.nativeElement as HTMLElement;
    expect(native.querySelector('[role="dialog"]')).toBeTruthy();
    expect(native.textContent).toContain('Fulfillment Stage');
    expect(native.textContent).toContain('Value and Weight');
    expect((native.querySelector('textarea') as HTMLTextAreaElement).value).toContain(
      "event.type = 'failed delivery'",
    );
  });
});

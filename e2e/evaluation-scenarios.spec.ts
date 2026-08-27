import { readFileSync } from 'node:fs';
import { expect, test, type Page } from '@playwright/test';
import { fixturePath } from './fixtures';

interface ScenarioCase {
  scenario: 'inventory' | 'manufacturing';
  fileName: string;
  presetId: string;
  leadingType: string;
  expectedTransition: string;
}

const scenarios: ScenarioCase[] = [
  {
    scenario: 'inventory',
    fileName: 'inventory_smoke.json',
    presetId: 'evaluation-inventory-policy',
    leadingType: 'ItemLocation',
    expectedTransition: 'Normal -> Understock',
  },
  {
    scenario: 'manufacturing',
    fileName: 'manufacturing_smoke.json',
    presetId: 'evaluation-manufacturing-operation',
    leadingType: 'Machine',
    expectedTransition: 'Running -> Degraded',
  },
];

async function importAndApplyState(page: Page, scenario: ScenarioCase): Promise<void> {
  await page.goto('/');
  await page.getByTestId('ocel-upload').setInputFiles({
    name: scenario.fileName,
    mimeType: 'application/json',
    buffer: readFileSync(fixturePath(scenario.scenario)),
  });
  await expect(page.getByRole('heading', { name: 'Statistics' })).toBeVisible();
  await page.getByTestId('open-state-dialog').click();
  await page.getByTestId(`state-preset-${scenario.presetId}`).click();
  await page.getByTestId('apply-state-query').click();
  await expect(page.getByTestId('state-message')).toContainText(`for ${scenario.leadingType}`);
  await expect(page.getByRole('heading', { name: 'State Patterns' })).toBeVisible();
}

for (const scenario of scenarios) {
  test(`${scenario.scenario} smoke supports the full state-aware workflow`, async ({ page }) => {
    await importAndApplyState(page, scenario);

    await page.getByTestId('patterns-inter-tab').click();
    await expect(page.getByTestId('inter-pattern-select')).toHaveValue(/inter-/);
    await expect(page.getByTestId('selected-inter-pattern')).toContainText('Support');

    await page.getByTestId('feature-sa-ocdfg').click();
    const graph = page.getByTestId('sa-ocdfg');
    await expect(graph).toBeVisible();
    await expect(graph).toContainText('CHANGE');

    await page.getByTestId('feature-transition-kpis').click();
    const kpis = page.getByTestId('transition-kpis');
    await expect(kpis).toBeVisible();
    await expect(kpis).toContainText(scenario.expectedTransition);

    await page.getByTestId('feature-state-detection').click();
    await page.getByTestId('state-detection-object-type').selectOption(scenario.leadingType);
    await page
      .getByTestId('state-detection-window')
      .fill(scenario.scenario === 'inventory' ? '3' : '4');
    await page.getByTestId('run-state-detection').click();
    const populatedCell = page.locator('[data-testid="som-cell"]:not(.is-empty)').first();
    await expect(populatedCell).toBeVisible();
    await populatedCell.click();
    await expect(page.getByTestId('som-cell-detail')).toBeVisible();
    await page.getByTestId('som-cell-detail').getByRole('button', { name: 'Close' }).click();

    await page.getByTestId('export-menu-button').click();
    const downloadPromise = page.waitForEvent('download');
    await page.getByTestId('export-json').click();
    const download = await downloadPromise;
    const stream = await download.createReadStream();
    const chunks: Buffer[] = [];
    for await (const chunk of stream) {
      chunks.push(Buffer.from(chunk));
    }
    const exported = JSON.parse(Buffer.concat(chunks).toString('utf8')) as {
      events: Array<{ attributes: Array<{ name: string }> }>;
    };
    expect(
      exported.events.some((event) => event.attributes.some((item) => item.name === 'state')),
    ).toBe(true);
  });
}

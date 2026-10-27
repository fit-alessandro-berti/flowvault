import { readFileSync } from 'node:fs';
import { expect, test, type Page } from '@playwright/test';

async function exportStates(page: Page): Promise<string[]> {
  await page.getByTestId('export-menu-button').click();
  const downloadPromise = page.waitForEvent('download');
  await page.getByTestId('export-json').click();
  const stream = await (await downloadPromise).createReadStream();
  const chunks: Buffer[] = [];
  for await (const chunk of stream) chunks.push(Buffer.from(chunk));
  const log = JSON.parse(Buffer.concat(chunks).toString('utf8')) as {
    events: Array<{ attributes: Array<{ name: string; value: string }> }>;
  };
  return log.events.flatMap((event) =>
    event.attributes.filter((attr) => attr.name === 'state').map((attr) => attr.value),
  );
}

test('stock quantile preset classifies and recomputes custom percentile cutoffs', async ({
  page,
}, testInfo) => {
  const log = {
    eventTypes: [{ name: 'Measure stock', attributes: [{ name: 'Stock After', type: 'string' }] }],
    objectTypes: [{ name: 'MAT', attributes: [] }],
    objects: [{ id: 'material', type: 'MAT', attributes: [], relationships: [] }],
    events: Array.from({ length: 100 }, (_, index) => ({
      id: `e${index}`,
      type: 'Measure stock',
      time: new Date(Date.UTC(2025, 0, 1, 0, index)).toISOString(),
      attributes: [{ name: 'Stock After', value: String(index + 1) }],
      relationships: [{ objectId: 'material', qualifier: '' }],
    })),
  };
  const errors: string[] = [];
  page.on('pageerror', (error) => errors.push(error.message));
  await page.goto('/');
  await page
    .getByTestId('ocel-upload')
    .setInputFiles({
      name: 'inventory_management_simulated.json',
      mimeType: 'application/json',
      buffer: Buffer.from(JSON.stringify(log)),
    });
  await page.getByTestId('open-state-dialog').click();
  await page.getByTestId('state-preset-inventory-stock-quantiles').click();
  await expect(page.getByLabel('SQL-like state query')).toHaveValue(
    /QUANTILE\(event\."Stock After", 0\.9\)/,
  );
  await page.screenshot({ path: testInfo.outputPath('quantile-preset.png'), fullPage: true });
  await page.getByTestId('apply-state-query').click();
  await expect(page.getByTestId('state-dialog')).not.toBeVisible();
  let states = await exportStates(page);
  expect(states.filter((state) => state === 'Understock')).toHaveLength(10);
  expect(states.filter((state) => state === 'Normal')).toHaveLength(80);
  expect(states.filter((state) => state === 'Overstock')).toHaveLength(10);

  await page.getByTestId('open-state-dialog').click();
  const query = await page.getByLabel('SQL-like state query').inputValue();
  await page
    .getByLabel('SQL-like state query')
    .fill(query.replaceAll('0.1)', '0.25)').replaceAll('0.9)', '0.75)'));
  await page.getByTestId('apply-state-query').click();
  states = await exportStates(page);
  expect(states.filter((state) => state === 'Understock')).toHaveLength(25);
  expect(states.filter((state) => state === 'Normal')).toHaveLength(50);
  expect(states.filter((state) => state === 'Overstock')).toHaveLength(25);

  await page.getByTestId('open-state-dialog').click();
  await page.getByLabel('SQL-like state query').fill(query.replaceAll('0.9)', '90)'));
  await page.getByTestId('apply-state-query').click();
  await expect(page.getByTestId('state-dialog').getByRole('alert')).toContainText(
    'numeric probability from 0 to 1',
  );
  await page
    .getByTestId('state-dialog')
    .getByRole('button', { name: 'Cancel', exact: true })
    .last()
    .click();
  expect(await exportStates(page)).toEqual(states);
  expect(errors).toEqual([]);
});

for (const scenario of ['inventory', 'manufacturing'] as const) {
  test(`${scenario} quantile example is offered for observed.ocel.json`, async ({ page }) => {
    await page.goto('/');
    await page
      .getByTestId('ocel-upload')
      .setInputFiles({
        name: 'observed.ocel.json',
        mimeType: 'application/json',
        buffer: readFileSync(`files/ocel2/evaluation/${scenario}_golden/observed.ocel.json`),
      });
    await page.getByTestId('open-state-dialog').click();
    await page
      .getByTestId(
        scenario === 'inventory'
          ? 'state-preset-evaluation-inventory-stock-quantiles'
          : 'state-preset-evaluation-manufacturing-health-quantiles',
      )
      .click();
    await page.getByTestId('apply-state-query').click();
    await expect(page.getByTestId('state-dialog')).not.toBeVisible();
    const states = await exportStates(page);
    expect(states).toContain(scenario === 'inventory' ? 'Understock' : 'Low Health');
    expect(states).toContain(scenario === 'inventory' ? 'Overstock' : 'High Health');
  });
}

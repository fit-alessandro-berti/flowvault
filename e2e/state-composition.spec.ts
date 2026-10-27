import { readFileSync } from 'node:fs';
import { expect, test, type Page } from '@playwright/test';

async function openEditor(page: Page): Promise<void> {
  await page.getByTestId('open-state-dialog').click();
  await expect(page.getByTestId('state-dialog')).toBeVisible();
}

async function exportLog(page: Page): Promise<any> {
  await page.getByTestId('export-menu-button').click();
  const downloadPromise = page.waitForEvent('download');
  await page.getByTestId('export-json').click();
  const stream = await (await downloadPromise).createReadStream();
  const chunks: Buffer[] = [];
  for await (const chunk of stream) chunks.push(Buffer.from(chunk));
  return JSON.parse(Buffer.concat(chunks).toString('utf8'));
}

function attr(log: any, event: number, name: string): unknown {
  return log.events[event].attributes.find((item: any) => item.name === name)?.value;
}

test('independent states can be composed, edited later, and exported', async ({
  page,
}, testInfo) => {
  const errors: string[] = [];
  page.on('pageerror', (error) => errors.push(error.message));
  await page.goto('/');
  await page.getByTestId('ocel-upload').setInputFiles({
    name: 'multiple_states.json',
    mimeType: 'application/json',
    buffer: readFileSync('rust/ocel_core/tests/fixtures/multiple_states.json'),
  });
  await openEditor(page);
  await page.getByTestId('add-state-attribute').click();
  await expect(page.getByLabel('State attribute name', { exact: true })).toHaveValue('state1');
  await page
    .getByLabel('SQL-like state query')
    .fill(
      "STATE state1 FOR LEADING OBJECT TYPE 'Order' AS CASE WHEN object.status IS NOT NULL THEN object.status END",
    );
  await page.getByTestId('apply-state-query').click();
  await expect(page.getByTestId('state-message')).toContainText('Added state1 for Order');

  await openEditor(page);
  await page.getByTestId('add-state-attribute').click();
  await expect(page.getByLabel('State attribute name', { exact: true })).toHaveValue('state2');
  await page.getByLabel('Leading object type', { exact: true }).selectOption('Shipment');
  await page
    .getByLabel('SQL-like state query')
    .fill(
      "STATE state2 FOR LEADING OBJECT TYPE 'Shipment' AS CASE WHEN object.status IS NOT NULL THEN object.status END",
    );
  await page.getByTestId('apply-state-query').click();
  const before = await exportLog(page);
  expect(attr(before, 0, 'state1')).toBe('Open');
  expect(attr(before, 0, 'state2')).toBe('Transit');
  expect(attr(before, 1, 'state2')).toBeUndefined();

  await openEditor(page);
  await page.getByTestId('state-composition-tab').click();
  await page.getByRole('button', { name: 'Combine states only', exact: true }).click();
  await expect(page.getByLabel('Combined state expression')).toHaveValue(
    'CONCAT(event."state1", \' / \', event."state2")',
  );
  await page
    .getByLabel('Combined state expression')
    .fill("CONCAT(event.state1, ' / ', COALESCE(event.state2, 'Unknown'))");
  await page.screenshot({ path: testInfo.outputPath('composition-editor.png'), fullPage: true });
  await page.getByTestId('apply-state-composition').click();
  await expect(page.getByTestId('sa-ocdfg')).toContainText('Open / Transit');
  await expect(page.getByTestId('sa-ocdfg')).toContainText('Open / Unknown');
  await page.screenshot({ path: testInfo.outputPath('composed-graph.png'), fullPage: true });

  // Reopening keeps the saved composition; invalid changes leave it intact.
  await openEditor(page);
  await page.getByTestId('state-composition-tab').click();
  await expect(page.getByLabel('Activity label expression')).toHaveValue('event.combined_state');
  await page.getByLabel('Activity label expression').fill('event.misspelled');
  await page.getByTestId('apply-state-composition').click();
  await expect(page.getByTestId('state-dialog').getByRole('alert')).toContainText(
    "unknown event attribute 'misspelled'",
  );
  await page
    .getByLabel('Activity label expression')
    .fill("CONCAT(event.type, ': ', event.combined_state)");
  await page.getByTestId('apply-state-composition').click();
  await expect(page.getByTestId('sa-ocdfg')).toContainText('Act: Open / Transit');

  await openEditor(page);
  await page.getByRole('button', { name: 'state2 · Shipment', exact: true }).click();
  await page
    .getByLabel('SQL-like state query')
    .fill(
      "STATE state2 FOR LEADING OBJECT TYPE 'Shipment' AS CASE WHEN object.status IS NOT NULL THEN CONCAT('Ship-', object.status) END",
    );
  await page.getByTestId('apply-state-query').click();
  await page.getByTestId('feature-sa-ocdfg').click();
  await expect(page.getByTestId('sa-ocdfg')).toContainText('Finish: Closed / Ship-Delayed');
  const result = await exportLog(page);
  expect(attr(result, 0, 'state1')).toBe('Open');
  expect(attr(result, 0, 'state2')).toBe('Ship-Transit');
  expect(attr(result, 0, 'state_activity')).toBe('Act: Open / Ship-Transit');
  expect(result.events[0].type).toBe('Act');
  expect(errors).toEqual([]);
});

test('states with no assigned values can be saved and composed later', async ({ page }) => {
  await page.goto('/');
  await page.getByTestId('ocel-upload').setInputFiles({
    name: 'multiple_states.json',
    mimeType: 'application/json',
    buffer: readFileSync('rust/ocel_core/tests/fixtures/multiple_states.json'),
  });
  await openEditor(page);
  await page.getByTestId('add-state-attribute').click();
  await page
    .getByLabel('SQL-like state query')
    .fill(
      "STATE state1 FOR LEADING OBJECT TYPE 'Order' AS CASE WHEN event.type = 'Absent' THEN 'Never' END",
    );
  await page.getByTestId('apply-state-query').click();
  await expect(page.getByTestId('state-dialog')).not.toBeVisible();
  await expect(page.getByTestId('state-message')).toContainText('to 0 of 3 events');
  await openEditor(page);
  await page.getByRole('button', { name: 'state1 · Order', exact: true }).click();
  await expect(page.getByLabel('SQL-like state query')).toHaveValue(/event.type = 'Absent'/);
  await page.getByTestId('state-composition-tab').click();
  await page.getByLabel('Combined state expression').fill("COALESCE(event.state1, 'Unknown')");
  await page.getByLabel('Activity label expression').fill('event.combined_state');
  await page.getByTestId('apply-state-composition').click();
  await expect(page.getByTestId('sa-ocdfg')).toContainText('Unknown');
  await expect(page.getByTestId('state-message')).toContainText('for 3 of 3 events');
});

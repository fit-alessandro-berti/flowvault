import { STATIC_SAMPLE_LOGS } from './helpers/static-data';
import { presetsForFile, STATE_QUERY_PRESETS } from './state-query-presets';

describe('state query presets', () => {
  it('provides four presets for each fixture log', () => {
    expect(presetsForFile('ocel20_example.json').length).toBe(4);
    expect(presetsForFile('container_logistics.xml').length).toBe(4);
    expect(presetsForFile('order-management.json').length).toBe(4);
    expect(presetsForFile('inventory_management_simulated.xml').length).toBe(4);
    expect(presetsForFile('order-management.json.gz').length).toBe(4);
    expect(presetsForFile('inventory_management_simulated.xml.gz').length).toBe(4);
  });

  it('offers a quantile example for every sample format and evaluation log family', () => {
    const files = [
      ...STATIC_SAMPLE_LOGS.map((sample) => sample.fileName),
      'inventory_smoke.json',
      'manufacturing_paper.ocel.json',
    ];
    for (const file of files) {
      const examples = presetsForFile(file).filter((preset) => preset.query.includes('QUANTILE('));
      expect(examples, file).toHaveLength(1);
      expect(examples[0].query).toContain(', 0.1)');
      expect(examples[0].query).toContain(', 0.9)');
    }
  });

  it('recognizes evaluation logs with the shared observed filename from their object types', () => {
    expect(
      presetsForFile('observed.ocel.json', ['ItemLocation', 'Material']).map((preset) => preset.id),
    ).toContain('evaluation-inventory-stock-quantiles');
    expect(
      presetsForFile('observed.ocel.json', ['Machine', 'ProductionOrder']).map(
        (preset) => preset.id,
      ),
    ).toContain('evaluation-manufacturing-health-quantiles');
  });

  it('uses named SQL-like state queries', () => {
    for (const preset of STATE_QUERY_PRESETS) {
      expect(preset.name.length).toBeGreaterThan(4);
      expect(preset.leadingObjectType.length).toBeGreaterThan(0);
      expect(preset.query).toContain(
        `STATE state FOR LEADING OBJECT TYPE '${preset.leadingObjectType}' AS CASE`,
      );
      expect(preset.query).toContain('END');
    }
  });

  it('selects the registered evaluation queries without shadowing legacy inventory logs', () => {
    expect(presetsForFile('inventory_smoke.json').map((preset) => preset.id)).toEqual([
      'evaluation-inventory-policy',
      'evaluation-inventory-stock-quantiles',
    ]);
    expect(presetsForFile('manufacturing_paper.ocel.json').map((preset) => preset.id)).toEqual([
      'evaluation-manufacturing-operation',
      'evaluation-manufacturing-health-quantiles',
    ]);
    expect(presetsForFile('inventory_management_simulated.xml')).toHaveLength(4);
  });
});

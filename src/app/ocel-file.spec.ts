import { exportBaseName, formatHintForFile } from './ocel-file';

describe('OCEL file helpers', () => {
  it('detects JSON OCEL filenames', () => {
    expect(formatHintForFile('sample.json')).toBe('json');
    expect(formatHintForFile('sample.JSONOCEL')).toBe('json');
    expect(formatHintForFile('sample.json.gz')).toBe('json');
    expect(formatHintForFile('sample.JSONOCEL.GZ')).toBe('json');
  });

  it('detects XML OCEL filenames', () => {
    expect(formatHintForFile('sample.xml')).toBe('xml');
    expect(formatHintForFile('sample.XMLOCEL')).toBe('xml');
    expect(formatHintForFile('sample.xml.gz')).toBe('xml');
    expect(formatHintForFile('sample.XMLOCEL.GZ')).toBe('xml');
  });

  it('detects CSV, SQLite, and bundled OCEL filenames', () => {
    expect(formatHintForFile('sample.OCEL.CSV')).toBe('csv');
    expect(formatHintForFile('sample.sqlite')).toBe('sqlite');
    expect(formatHintForFile('sample.SQLITE3')).toBe('sqlite');
    expect(formatHintForFile('sample.OCEL.ZIP')).toBe('bundle');
  });

  it('falls back when no known extension exists', () => {
    expect(formatHintForFile('sample.txt')).toBeUndefined();
  });

  it('uses the source name for exports without duplicating extensions', () => {
    expect(exportBaseName('purchase-to-pay.jsonocel')).toBe('purchase-to-pay');
    expect(exportBaseName('orders.xml')).toBe('orders');
    expect(exportBaseName('orders.xml.gz')).toBe('orders');
    expect(exportBaseName('Purchase.JSON.GZ')).toBe('Purchase');
    expect(exportBaseName('purchase-to-pay.ocel.csv')).toBe('purchase-to-pay');
    expect(exportBaseName('purchase-to-pay.sqlite')).toBe('purchase-to-pay');
    expect(exportBaseName('purchase-to-pay.sqlite3')).toBe('purchase-to-pay');
    expect(exportBaseName('purchase-to-pay.ocel.zip')).toBe('purchase-to-pay');
    expect(exportBaseName('')).toBe('ocel-export');
  });
});

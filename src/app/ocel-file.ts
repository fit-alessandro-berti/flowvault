export type OcelFormatHint = 'json' | 'xml' | 'csv' | 'sqlite' | 'bundle' | undefined;

const JSON_EXTENSIONS = ['.json', '.jsonocel'];
const XML_EXTENSIONS = ['.xml', '.xmlocel'];

export function formatHintForFile(fileName: string): OcelFormatHint {
  const normalized = normalizedSourceName(fileName).toLowerCase();

  if (JSON_EXTENSIONS.some((extension) => normalized.endsWith(extension))) {
    return 'json';
  }

  if (XML_EXTENSIONS.some((extension) => normalized.endsWith(extension))) {
    return 'xml';
  }

  if (normalized.endsWith('.ocel.csv')) {
    return 'csv';
  }

  if (normalized.endsWith('.sqlite') || normalized.endsWith('.sqlite3')) {
    return 'sqlite';
  }

  if (normalized.endsWith('.ocel.zip')) {
    return 'bundle';
  }

  return undefined;
}

export function exportBaseName(fileName: string): string {
  const trimmed = normalizedSourceName(fileName);
  if (!trimmed) {
    return 'ocel-export';
  }

  return (
    trimmed.replace(/\.(ocel\.(csv|zip)|jsonocel|xmlocel|sqlite3?|json|xml)$/i, '') || 'ocel-export'
  );
}

function normalizedSourceName(fileName: string): string {
  return fileName.trim().replace(/\.gz$/i, '');
}

export type OcelFormatHint = 'json' | 'xml' | undefined;

const JSON_EXTENSIONS = ['.json', '.jsonocel'];
const XML_EXTENSIONS = ['.xml', '.xmlocel'];

export function formatHintForFile(fileName: string): OcelFormatHint {
  const normalized = fileName.trim().toLowerCase();

  if (JSON_EXTENSIONS.some((extension) => normalized.endsWith(extension))) {
    return 'json';
  }

  if (XML_EXTENSIONS.some((extension) => normalized.endsWith(extension))) {
    return 'xml';
  }

  return undefined;
}

export function exportBaseName(fileName: string): string {
  const trimmed = fileName.trim();
  if (!trimmed) {
    return 'ocel-export';
  }

  return trimmed.replace(/\.(jsonocel|xmlocel|json|xml)$/i, '') || 'ocel-export';
}

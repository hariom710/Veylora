import type { FileInfo } from './types';

export type SortKey = 'name' | 'date' | 'size';

// Sort a copy of the list for the File Manager grid: name = A→Z,
// date/size = newest/largest first. Never mutates the input.
export function sortFiles(files: FileInfo[], key: SortKey): FileInfo[] {
  const out = [...files];
  if (key === 'name') {
    out.sort((a, b) => a.filename.localeCompare(b.filename, undefined, { sensitivity: 'base' }));
  } else if (key === 'date') {
    out.sort((a, b) => Number(b.created_at) - Number(a.created_at));
  } else {
    out.sort((a, b) => b.encrypted_size - a.encrypted_size);
  }
  return out;
}

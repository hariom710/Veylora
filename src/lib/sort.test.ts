import { describe, it, expect } from 'vitest';
import { sortFiles } from './sort';
import type { FileInfo } from './types';

function file(id: number, filename: string, created_at: string, encrypted_size: number): FileInfo {
  return {
    id,
    filename,
    file_id: String(id).repeat(32).slice(0, 32),
    folder_id: null,
    encrypted_size,
    created_at,
    modified_at: created_at,
  };
}

const FILES = [
  file(1, 'video.mp4', '100', 500),
  file(2, 'document.pdf', '300', 100),
  file(3, 'Photo.JPG', '200', 300),
];

describe('sortFiles', () => {
  it('sorts by name A→Z, case-insensitively', () => {
    const out = sortFiles(FILES, 'name');
    expect(out.map((f) => f.filename)).toEqual(['document.pdf', 'Photo.JPG', 'video.mp4']);
  });

  it('sorts by date, newest first', () => {
    const out = sortFiles(FILES, 'date');
    expect(out.map((f) => f.id)).toEqual([2, 3, 1]);
  });

  it('sorts by size, largest first', () => {
    const out = sortFiles(FILES, 'size');
    expect(out.map((f) => f.id)).toEqual([1, 3, 2]);
  });

  it('does not mutate the input array', () => {
    const input = [...FILES];
    sortFiles(input, 'name');
    expect(input.map((f) => f.id)).toEqual([1, 2, 3]);
  });
});

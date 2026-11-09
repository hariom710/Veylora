import { describe, it, expect } from 'vitest';
import { vaultUrl, vaultUrlOpenEnded } from './vaultUrl';

describe('vaultUrl', () => {
  const token = 'abc123def456';
  const fileId = '0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef';

  it('builds URL without range', () => {
    const url = vaultUrl(fileId, token);
    expect(url).toBe(`vault://file/${fileId}?token=${token}`);
  });

  it('builds URL with closed range', () => {
    const url = vaultUrl(fileId, token, [100, 200]);
    expect(url).toBe(`vault://file/${fileId}?token=${token}&range=100-200`);
  });

  it('builds URL with open range', () => {
    const url = vaultUrlOpenEnded(fileId, token, 100);
    expect(url).toBe(`vault://file/${fileId}?token=${token}&range=100-`);
  });

  it('throws if token is empty', () => {
    expect(() => vaultUrl(fileId, '')).toThrow('Token is required');
    expect(() => vaultUrlOpenEnded(fileId, '', 0)).toThrow('Token is required');
  });
});

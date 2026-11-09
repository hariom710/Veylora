export function vaultUrl(
  fileIdHex: string,
  token: string,
  range?: [number, number],
): string {
  if (!token) {
    throw new Error('Token is required for vault:// URL');
  }
  const base = `vault://file/${fileIdHex}?token=${encodeURIComponent(token)}`;
  if (range) {
    return `${base}&range=${range[0]}-${range[1]}`;
  }
  return base;
}

export function vaultUrlOpenEnded(
  fileIdHex: string,
  token: string,
  start: number,
): string {
  if (!token) {
    throw new Error('Token is required for vault:// URL');
  }
  return `vault://file/${fileIdHex}?token=${encodeURIComponent(token)}&range=${start}-`;
}

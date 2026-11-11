// WebView2 cannot intercept the raw vault:// scheme for subresources — wry
// registers the WebResourceRequested filter as http://vault.*. The page must
// therefore request the http://vault.localhost/file/... form: it matches the
// filter, stays on a trustworthy local origin (no mixed-content block), and
// wry reverts it to vault://localhost/... — main.rs strips the "localhost/"
// segment so the frozen streaming parser sees vault://file/... as designed.
const VAULT_BASE = 'http://vault.localhost/file/';

export function vaultUrl(
  fileIdHex: string,
  token: string,
  range?: [number, number],
): string {
  if (!token) {
    throw new Error('Token is required for vault:// URL');
  }
  const base = `${VAULT_BASE}${fileIdHex}?token=${encodeURIComponent(token)}`;
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
  return `${VAULT_BASE}${fileIdHex}?token=${encodeURIComponent(token)}&range=${start}-`;
}

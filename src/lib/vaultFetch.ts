export async function vaultFetch(
  url: string,
  onUnauthorized: () => void,
  options?: RequestInit,
): Promise<Response> {
  const res = await fetch(url, options);
  if (res.status === 403) {
    onUnauthorized();
  }
  return res;
}

export function buildAccessUrl(
  ip: string,
  port: number,
  token: string,
  tokenRequired: boolean,
) {
  if (!ip) return "";
  const url = new URL(`http://${ip}:${port}/`);
  if (tokenRequired && token) url.searchParams.set("token", token);
  return url.toString();
}

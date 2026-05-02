const KNOWN_HOMES = ['/home/', '/Users/', '/root/'];

export function formatPath(path: string | undefined | null): string {
  if (!path) return '';
  for (const root of KNOWN_HOMES) {
    if (!path.startsWith(root)) continue;
    const after = path.slice(root.length);
    const slash = after.indexOf('/');
    if (slash === -1) return '~';
    return '~' + after.slice(slash);
  }
  return path;
}

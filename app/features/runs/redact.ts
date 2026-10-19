// Defense in depth: the runner removes session secrets before sending a report.
// Export also masks conventional sensitive field names, including nested payloads.
const sensitive =
  /^(authorization|proxy-authorization|cookie|set-cookie|password|passwd|secret|token|access[_-]?token|refresh[_-]?token|api[_-]?key|x-api-key|client[_-]?secret)$/i;
export function redactReport<T>(report: T): T {
  const walk = (value: unknown, key = ''): unknown => {
    if (sensitive.test(key)) return '[REDACTED]';
    if (Array.isArray(value)) return value.map((item) => walk(item));
    if (value && typeof value === 'object')
      return Object.fromEntries(
        Object.entries(value).map(([name, child]) => [name, walk(child, name)]),
      );
    if (typeof value === 'string' && key === 'url') {
      try {
        const url = new URL(value, 'http://local.invalid');
        for (const name of url.searchParams.keys())
          if (sensitive.test(name)) url.searchParams.set(name, '[REDACTED]');
        if (url.username) url.username = '[REDACTED]';
        if (url.password) url.password = '[REDACTED]';
        return value.startsWith('http')
          ? url.toString()
          : `${url.pathname}${url.search}${url.hash}`;
      } catch {
        return value;
      }
    }
    return value;
  };
  return walk(report) as T;
}

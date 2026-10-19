import type { Json } from '../../../shared/model';
import { appendPath } from '../../../shared/graph';

// Reserved prototype keys remain deliberately inaccessible to the runner.
export function supportedKey(key: string): boolean {
  return !['__proto__', 'constructor', 'prototype'].includes(key);
}
export function childPath(parent: string, key: string, array: boolean): string {
  return appendPath(parent, array ? Number(key) : key);
}
export function samplePaths(value: Json | undefined, path = '', depth = 0): string[] {
  if (value === undefined || depth > 6 || value === null || typeof value !== 'object') return [];
  return Object.entries(value)
    .slice(0, 50)
    .flatMap(([key, child]) => {
      if (!Array.isArray(value) && !supportedKey(key)) return [];
      const next = childPath(path, key, Array.isArray(value));
      return [next, ...samplePaths(child, next, depth + 1)];
    })
    .slice(0, 150);
}

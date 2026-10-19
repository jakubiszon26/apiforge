import type { Json } from '../../../shared/model';
import { jsonType } from '../../../shared/operators';
import { childPath, supportedKey } from './jsonPaths';

export interface ResponseField {
  path: string;
  type: string;
  preview: string;
  depth: number;
}
export function collectResponseFields(value: Json | undefined, limit = 1000): ResponseField[] {
  if (value === undefined) return [];
  const fields: ResponseField[] = [];
  const visit = (value: Json, path: string, depth: number) => {
    if (fields.length >= limit) return;
    const type = jsonType(value);
    const preview =
      value !== null && typeof value === 'object'
        ? `${Array.isArray(value) ? 'Lista' : 'Obiekt'} · ${Object.keys(value).length}`
        : JSON.stringify(value).slice(0, 160);
    fields.push({ path, type, preview, depth });
    if (depth >= 12 || value === null || typeof value !== 'object') return;
    for (const [key, child] of Object.entries(value)) {
      if (fields.length >= limit) break;
      if (!Array.isArray(value) && !supportedKey(key)) continue;
      visit(child, childPath(path, key, Array.isArray(value)), depth + 1);
    }
  };
  visit(value, '', 0);
  return fields;
}

import test from 'node:test';
import assert from 'node:assert/strict';
import { collectResponseFields } from './responseFields';
import { getPath } from '../../../shared/graph';

test('selectable field paths preserve null, false, zero, arrays and nonstandard keys', () => {
  const sample = {
    'a.b': [{ count: 0, active: false, empty: null, 'inna nazwa': 'tekst', '': 42 }],
    constructor: 'reserved',
  };
  const fields = collectResponseFields(sample);
  assert.equal(fields[0].path, '');
  assert.ok(fields.some((field) => field.path === '["a.b"][0].empty' && field.type === 'null'));
  assert.ok(
    fields.some((field) => field.path === '["a.b"][0].active' && field.preview === 'false'),
  );
  assert.ok(fields.some((field) => field.path === '["a.b"][0].count' && field.preview === '0'));
  assert.ok(fields.some((field) => getPath(sample, field.path) === 42));
  assert.ok(!fields.some((field) => field.path === 'constructor'));
  for (const field of fields) assert.notEqual(getPath(sample, field.path), undefined, field.path);
  assert.deepEqual(collectResponseFields(undefined), []);
  assert.equal(collectResponseFields(null)[0].preview, 'null');
  assert.equal(collectResponseFields(Array(2000).fill(1)).length, 1000);
});

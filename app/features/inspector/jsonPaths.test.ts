import assert from 'node:assert/strict';
import test from 'node:test';
import { getPath } from '../../../shared/graph';
import { childPath, samplePaths, supportedKey } from './jsonPaths';

test('every offered simple object/list path resolves through the runner path helper', () => {
  const value = { data: { items: [{ id: 0, label: 'value' }] }, empty: null };
  assert.deepEqual(samplePaths(value), [
    'data',
    'data.items',
    'data.items[0]',
    'data.items[0].id',
    'data.items[0].label',
    'empty',
  ]);
  for (const path of samplePaths(value)) assert.notEqual(getPath(value, path), undefined);
});
test('quoted property paths distinguish literal dots, brackets, empty keys and Unicode', () => {
  const value = {
    'a.b': { id: 'literal' },
    a: { b: 'nested' },
    'items[0]': 123,
    '': false,
    zażółć: 42,
    constructor: { id: 1 },
  };
  const paths = samplePaths(value);
  assert.ok(paths.includes('["a.b"].id'));
  assert.ok(paths.includes('["items[0]"]'));
  assert.ok(paths.includes('[""]'));
  assert.ok(paths.includes('["zażółć"]'));
  assert.equal(getPath(value, 'a.b'), 'nested');
  assert.equal(getPath(value, '["a.b"].id'), 'literal');
  assert.equal(getPath(value, '[""]'), false);
  assert.equal(getPath(value, '["items[0]"]'), 123);
  for (const path of paths) assert.notEqual(getPath(value, path), undefined);
  assert.equal(supportedKey('constructor'), false);
  assert.ok(!paths.some((p) => p.includes('constructor')));
  assert.equal(childPath('root', 'a.b', false), 'root["a.b"]');
});

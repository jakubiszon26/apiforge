import test from 'node:test';
import assert from 'node:assert/strict';
import { createShopProject, createTicketsProject } from '../shared/demos';
import { appendPath, getPath, validateProject } from '../shared/graph';
import { literal, reference } from '../shared/model';
import { parseProject, parseWorkspace, publicProject } from '../app/features/workspace/storage';

test('both domain examples survive export/import with contracts and pinned revisions', () => {
  for (const project of [createShopProject(), createTicketsProject()]) {
    assert.deepEqual(validateProject(project), []);
    assert.deepEqual(parseProject(JSON.stringify(publicProject(project))), project);
  }
});
test('malformed imports report errors rather than crash or access invalid properties', () => {
  const p = createShopProject();
  const malformed = [
    null,
    [],
    {},
    { ...p, version: 2 },
    { ...p, components: [null] },
    { ...p, scenarios: [] },
    { ...p, scenarios: [{ ...p.scenarios[0], graph: { nodes: [null], edges: [] } }] },
    { ...p, contracts: [{ id: 'x', name: 'x', sample: undefined }] },
  ];
  for (const value of malformed) assert.ok(validateProject(value).length);
  assert.throws(() => parseProject(JSON.stringify({ ...p, version: 7 })), /wersja/);
});
test('missing reference ports, recursive definitions and duplicate node identifiers are rejected', () => {
  const p = createShopProject();
  p.scenarios[0].graph.nodes[1].inputs.product = reference('product', 'noSuchOutput');
  assert.ok(validateProject(p).some((e) => e.includes('wyjścia')));
  const cycle = createShopProject();
  cycle.components[0].graph.nodes = [
    {
      id: 'recursive',
      name: 'Recursive',
      kind: 'component',
      position: { x: 0, y: 0 },
      inputs: { id: literal(1) },
      config: { componentId: 'get-product', revision: 1 },
    },
  ];
  cycle.components[0].outputs[0].binding = reference('recursive', 'product');
  assert.ok(validateProject(cycle).some((e) => e.includes('Rekurencyjna')));
  const duplicate = createShopProject();
  duplicate.scenarios[0].graph.nodes.push(duplicate.scenarios[0].graph.nodes[0]);
  assert.ok(validateProject(duplicate).some((e) => e.includes('powtórzone ID')));
});
test('cleanup can refer to main outputs; main graph cannot refer forward into cleanup', () => {
  const p = createShopProject();
  assert.deepEqual(validateProject(p), []);
  p.scenarios[0].graph.nodes[0].inputs.id = reference('delete-cart', 'body', 'id');
  assert.ok(validateProject(p).some((e) => e.includes('źródłowego')));
});
test('secret names persist, their values do not; sanitization does not mutate source', () => {
  const p = createShopProject();
  p.environment.secretNames = ['token'];
  p.environment.variables = { token: 'private', region: 'pl' };
  const clean = publicProject(p);
  assert.deepEqual(clean.environment.variables, { region: 'pl' });
  assert.deepEqual(clean.environment.secretNames, ['token']);
  assert.equal(p.environment.variables.token, 'private');
});
test('workspace requires unique projects and an existing active project', () => {
  const p = createShopProject();
  assert.throws(
    () => parseWorkspace(JSON.stringify({ version: 1, activeId: 'missing', projects: [p] })),
    /aktywnego/,
  );
  assert.throws(
    () => parseWorkspace(JSON.stringify({ version: 1, activeId: p.id, projects: [p, p] })),
    /Powtórzone/,
  );
});
test('path lookup distinguishes absent, null, zero and empty text', () => {
  const v = { a: [{ nil: null, zero: 0, text: '' }] };
  assert.equal(getPath(v, 'a[0].nil'), null);
  assert.equal(getPath(v, 'a[0].zero'), 0);
  assert.equal(getPath(v, 'a[0].text'), '');
  assert.equal(getPath(v, 'a[0].missing'), undefined);
  assert.equal(getPath(v, 'constructor'), undefined);
});
test('field picking safely addresses dotted, empty, quoted and unicode JSON keys', () => {
  for (const key of ['a.b', '', 'x[0]', 'cytat"test', 'inna nazwa', 'żółć']) {
    const path = appendPath('data', key);
    assert.equal(getPath({ data: { [key]: 42 } }, path), 42);
  }
  assert.equal(getPath({ a: [{ id: 2 }] }, 'a[0].id'), 2);
  assert.equal(getPath({ a: 1 }, 'a.'), undefined);
});

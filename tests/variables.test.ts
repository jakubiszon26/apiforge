import type { Graph } from '../shared/model';
import { assignResponseVariable } from '../app/features/variables/operations';
import test from 'node:test';
import assert from 'node:assert/strict';
import { createServer } from 'node:http';
import type { AddressInfo } from 'node:net';
import { createNode, dependencies, getPath, inputPorts, validateProject } from '../shared/graph';
import { createEmptyProject } from '../shared/demos';
import { literal, reference } from '../shared/model';
import {
  bindVariableText,
  pruneTemplateInputs,
  readVariable,
  writeVariable,
  VARIABLE_MIME,
  type VariableBlock,
} from '../shared/variables';
import { runScenario } from '../api/runner';
import { parseProject } from '../app/features/workspace/storage';

const end = (value: string) => ({ value, start: value.length, end: value.length });
test('text variable drops preserve prefixes, reuse bindings, avoid collisions and remove unused dependencies', () => {
  const source = createNode('variable');
  source.name = 'code';
  source.inputs.value = literal(123);
  let node = createNode('request');
  node.inputs.code = literal('unrelated');
  const variable: VariableBlock = { label: 'code', binding: reference(source.id, 'value') };
  node = bindVariableText(node, variable, end('/items/'), (node, url) => ({
    ...node,
    config: { ...node.config, url },
  }));
  assert.equal(node.config.url, '/items/{{code_2}}');
  assert.deepEqual(node.inputs.code_2, variable.binding);
  assert.equal(node.config.templateInputs?.length, 1);
  assert.ok(inputPorts(node, createEmptyProject()).some((port) => port.name === 'code_2'));
  node = bindVariableText(
    node,
    variable,
    { value: 'prefix REPLACE suffix', start: 7, end: 14 },
    (node, text) => ({ ...node, config: { ...node.config, headers: { 'X-Code': text } } }),
  );
  assert.equal(node.config.headers?.['X-Code'], 'prefix {{code_2}} suffix');
  assert.equal(node.config.templateInputs?.length, 1);
  node.config.url = '/items';
  assert.ok(pruneTemplateInputs(node).inputs.code_2);
  node.config.headers = {};
  const cleaned = pruneTemplateInputs(node);
  assert.equal(cleaned.inputs.code_2, undefined);
  assert.deepEqual(cleaned.inputs.code, literal('unrelated'));
  assert.deepEqual(dependencies({ nodes: [source, node], edges: [] }, node.id), [source.id]);
});

test('variable drag payloads contain bindings only and reject malformed data', () => {
  const values = new Map<string, string>();
  const data: Pick<DataTransfer, 'setData' | 'getData' | 'effectAllowed'> = {
    setData: (key: string, value: string) => values.set(key, value),
    getData: (key: string) => values.get(key) ?? '',
    effectAllowed: 'copy',
  };
  writeVariable(data, { label: 'apiToken', binding: { kind: 'environment', name: 'apiToken' } });
  assert.deepEqual(readVariable(data), {
    label: 'apiToken',
    binding: { kind: 'environment', name: 'apiToken' },
  });
  assert.deepEqual(JSON.parse(values.get(VARIABLE_MIME)!), {
    label: 'apiToken',
    binding: { kind: 'environment', name: 'apiToken' },
  });
  for (const value of [
    'null',
    '{}',
    '{"label":"x","binding":{"kind":"literal","value":"secret"}}',
    '{"label":"x","binding":{"kind":"parameter","name":"__proto__"}}',
  ]) {
    values.set(VARIABLE_MIME, value);
    assert.equal(readVariable(data), undefined);
  }
});

test('variable blocks drive actual HTTP templates and typed JSON inputs on every run', async () => {
  const server = createServer(async (req, res) => {
    const chunks: Buffer[] = [];
    for await (const chunk of req) chunks.push(chunk);
    res.setHeader('Content-Type', 'application/json');
    res.end(
      JSON.stringify({
        url: req.url,
        code: req.headers['x-code'],
        body: JSON.parse(Buffer.concat(chunks).toString()),
      }),
    );
  });
  await new Promise<void>((resolve) => server.listen(0, '127.0.0.1', resolve));
  try {
    const project = createEmptyProject();
    project.environment.baseUrl = `http://127.0.0.1:${(server.address() as AddressInfo).port}`;
    project.scenarios[0].parameters = { region: 'pl' };
    const code = createNode('variable');
    code.name = 'code';
    code.inputs.value = literal(123);
    const payload = createNode('variable');
    payload.name = 'payload';
    payload.inputs.value = literal({ active: false, amount: 0, empty: null });
    let request = createNode('request');
    request.config.method = 'POST';
    request.inputs.body = reference(payload.id, 'value');
    const variable: VariableBlock = { label: code.name, binding: reference(code.id, 'value') };
    request = bindVariableText(request, variable, end('/items/'), (node, url) => ({
      ...node,
      config: { ...node.config, url },
    }));
    request = bindVariableText(request, variable, end('code='), (node, text) => ({
      ...node,
      config: { ...node.config, headers: { 'X-Code': text } },
    }));
    request = bindVariableText(
      request,
      { label: 'region', binding: { kind: 'parameter', name: 'region' } },
      end(''),
      (node, text) => ({ ...node, config: { ...node.config, query: { region: text } } }),
    );
    const comparison = createNode('assert');
    comparison.inputs = { actual: reference(code.id, 'value'), expected: literal(123) };
    project.scenarios[0].graph = { nodes: [request, comparison, code, payload], edges: [] };
    assert.deepEqual(validateProject(project), []);
    assert.deepEqual(parseProject(JSON.stringify(project)), project);
    const execute = () =>
      runScenario({ project, scenarioId: project.scenarios[0].id, mode: 'live' });
    const first = await execute();
    assert.equal(first.status, 'passed');
    const http = first.steps.find((step) => step.nodeId === request.id)!;
    assert.equal(getPath(http.outputs.body, 'url'), '/items/123?region=pl');
    assert.equal(getPath(http.outputs.body, 'code'), 'code=123');
    assert.deepEqual(getPath(http.outputs.body, 'body'), { active: false, amount: 0, empty: null });
    code.inputs.value = literal(456);
    const second = await execute();
    assert.equal(second.status, 'failed');
    assert.equal(second.steps.find((step) => step.nodeId === comparison.id)?.actual, 456);
    assert.equal(
      getPath(second.steps.find((step) => step.nodeId === request.id)!.outputs.body, 'url'),
      '/items/456?region=pl',
    );
    code.inputs.value = reference(comparison.id, 'value');
    assert.ok(validateProject(project).some((message) => message.includes('cykl')));
  } finally {
    await new Promise<void>((resolve) => server.close(() => resolve()));
  }
});

test('response assignments preserve zero, false, null and array paths and read fresh values', async () => {
  const project = createEmptyProject();
  project.environment.baseUrl = 'http://example.test';
  const source = createNode('request');
  source.config.sampleResponse = { items: [{ price: 0, flag: false, optional: null }] };
  let graph: Graph = { nodes: [source], edges: [] };
  const captures: [string, string, number | boolean | null][] = [
    ['capturedPrice', 'items[0].price', 0],
    ['capturedFlag', 'items[0].flag', false],
    ['capturedNull', 'items[0].optional', null],
  ];
  for (const [name, path, expected] of captures) {
    const result = assignResponseVariable(graph, project, source.id, 'body', path, name);
    assert.equal(result.error, undefined);
    graph = result.graph!;
    const comparison = createNode('assert');
    comparison.inputs = {
      actual: reference(result.variableId!, 'value'),
      expected: literal(expected),
    };
    graph = { ...graph, nodes: [...graph.nodes, comparison] };
    const variable = graph.nodes.find((node) => node.id === result.variableId)!;
    assert.deepEqual(variable.inputs.value, reference(source.id, 'body', path));
  }
  // The graph is deliberately stored out of execution order.
  project.scenarios[0].graph = { ...graph, nodes: [...graph.nodes].reverse() };
  assert.deepEqual(parseProject(JSON.stringify(project)), project);
  const run = () =>
    runScenario({ project, scenarioId: project.scenarios[0].id, mode: 'simulation' });
  assert.equal((await run()).status, 'passed');
  source.config.sampleResponse = { items: [{ price: 42, flag: false, optional: null }] };
  const result = await run();
  assert.equal(result.status, 'failed');
  assert.equal(result.steps.find((step) => step.name === 'capturedPrice')?.outputs.value, 42);
  assert.equal(result.steps.find((step) => step.name === 'capturedFlag')?.outputs.value, false);
  assert.equal(result.steps.find((step) => step.name === 'capturedNull')?.outputs.value, null);
  source.config.sampleResponse = { items: [{ price: 42, flag: false }] };
  const missing = await run();
  assert.equal(missing.steps.find((step) => step.name === 'capturedNull')?.status, 'error');
});

test('assigning an existing variable changes its binding and rejects cycles without mutations', () => {
  const project = createEmptyProject();
  const source = createNode('request');
  const variable = createNode('variable');
  const graph = { nodes: [source, variable], edges: [] };
  const before = structuredClone(graph);
  const captured = assignResponseVariable(
    graph,
    project,
    source.id,
    'body',
    'data.price',
    'price',
    variable.id,
  );
  assert.equal(captured.graph?.nodes.length, 2);
  assert.deepEqual(
    captured.graph?.nodes[1].inputs.value,
    reference(source.id, 'body', 'data.price'),
  );
  assert.deepEqual(graph, before);
  source.inputs.argument = reference(variable.id, 'value');
  assert.match(
    assignResponseVariable(graph, project, source.id, 'body', 'data.price', 'price', variable.id)
      .error!,
    /cykl/,
  );
  assert.deepEqual(variable.inputs.value, literal(''));
  assert.match(
    assignResponseVariable(graph, project, 'missing', 'body', '', 'x').error!,
    /dostępne/,
  );
  assert.match(
    assignResponseVariable(graph, project, source.id, 'missing', '', 'x').error!,
    /dostępne/,
  );
});

import test from 'node:test';
import assert from 'node:assert/strict';
import { createServer } from 'node:http';
import type { AddressInfo } from 'node:net';
import type { Binding, Graph, Json, TestNode } from '../shared/model';
import { literal, reference } from '../shared/model';
import { operators, evaluateOperator, expression, type Operator } from '../shared/operators';
import {
  createNode,
  dependencies,
  getPath,
  validateProject,
  visualEdges,
  deleteNodes,
} from '../shared/graph';
import { createBlocksProject, createEmptyProject } from '../shared/demos';
import { runScenario } from '../api/runner';
import { parseProject } from '../app/features/workspace/storage';
import { duplicateSelection, removeConnections } from '../app/features/graph/operations';
import {
  initializeInputs,
  initializeOutputs,
  materializeComponent,
} from '../app/features/components/componentDraft';

const op = (operator: Operator, left: Binding, right?: Binding): Binding => ({
  kind: 'expression',
  operator,
  inputs: { left, ...(right ? { right } : {}) },
});
const evaluate = (operator: Operator, left: Json, right?: Json) =>
  evaluateOperator(operator, { left, right }, getPath);

test('every operator family executes defined examples, with JSON equality and Scratch angles', () => {
  const examples: [Operator, Json, Json | undefined, Json][] = [
    ['add', 2, 3, 5],
    ['subtract', 2, 3, -1],
    ['multiply', 2, 3, 6],
    ['divide', 6, 3, 2],
    ['modulo', -5, 3, 1],
    ['round', 1.234, 2, 1.23],
    ['equals', { a: 1, b: 2 }, { b: 2, a: 1 }, true],
    ['notEquals', 1, '1', true],
    ['greater', 2, 1, true],
    ['less', 1, 2, true],
    ['atLeast', 2, 2, true],
    ['atMost', 2, 2, true],
    ['and', true, false, false],
    ['or', false, true, true],
    ['not', true, undefined, false],
    ['concat', 'hello ', 123, 'hello 123'],
    ['letter', 'a😀b', 2, '😀'],
    ['length', ['a', 'b'], undefined, 2],
    ['contains', [{ a: 1 }], { a: 1 }, true],
    ['abs', -5, undefined, 5],
    ['floor', 1.9, undefined, 1],
    ['ceiling', 1.1, undefined, 2],
    ['sqrt', 4, undefined, 2],
    ['sin', 90, undefined, 1],
    ['cos', 180, undefined, -1],
    ['tan', 0, undefined, 0],
    ['asin', 1, undefined, 90],
    ['acos', 1, undefined, 0],
    ['atan', 0, undefined, 0],
    ['ln', 1, undefined, 0],
    ['log', 100, undefined, 2],
    ['exp', 0, undefined, 1],
    ['power10', 2, undefined, 100],
    ['typeOf', null, undefined, 'null'],
    ['exists', false, undefined, true],
    ['field', { a: [{ value: 42 }] }, 'a[0].value', 42],
    ['random', 7, 7, 7],
  ];
  assert.equal(new Set(examples.map(([name]) => name)).size, Object.keys(operators).length);
  for (const [name, left, right, result] of examples)
    assert.deepEqual(evaluate(name, left, right), result, name);
  assert.equal(evaluate('length', 'a😀b'), 3);
  assert.equal(evaluate('contains', 'Hello', 'hello'), false);
  assert.equal(evaluateOperator('exists', { left: undefined }, getPath), false);
  assert.equal(evaluate('typeOf', []), 'array');
  assert.equal(evaluate('typeOf', {}), 'object');
  assert.equal(evaluate('field', {}, 'missing'), undefined);
  for (let i = 0; i < 40; i++) {
    const value = evaluate('random', 10, 1) as number;
    assert.ok(Number.isInteger(value) && value >= 1 && value <= 10);
  }
});

test('operators reject coercion, missing operands, undefined math and oversized results', () => {
  for (const [name, a, b] of [
    ['add', '2', 3],
    ['multiply', null, 2],
    ['and', 'true', true],
    ['divide', 1, 0],
    ['modulo', 1, 0],
    ['sqrt', -1, undefined],
    ['tan', 90, undefined],
    ['ln', 0, undefined],
    ['round', 1, -1],
    ['letter', 'text', 0],
    ['length', {}, undefined],
    ['contains', 12, '2'],
  ] as [Operator, Json, Json | undefined][])
    assert.throws(() => evaluate(name, a, b), Error, name);
  assert.throws(() => evaluateOperator('add', { left: 1 }, getPath), /argument/);
  assert.throws(() => evaluate('concat', 'a'.repeat(100000), 'b'), /limit/);
});

test('nested references drive ordering, persistence, duplication and precise disconnects', async () => {
  const project = createBlocksProject();
  assert.deepEqual(validateProject(project), []);
  assert.deepEqual(parseProject(JSON.stringify(project)), project);
  const graph = project.scenarios[0].graph;
  assert.deepEqual(dependencies(graph, 'name-check'), ['response']);
  const edges = visualEdges(graph).filter((edge) => edge.target === 'name-check');
  assert.equal(edges.length, 2);
  assert.notEqual(edges[0].id, edges[1].id);
  const duplicate = duplicateSelection(graph, ['response', 'name-check']);
  assert.deepEqual(dependencies(duplicate.graph, duplicate.ids[1]), [duplicate.ids[0]]);
  const disconnected = removeConnections(graph, [edges[0].id]);
  assert.equal(visualEdges(disconnected).filter((edge) => edge.target === 'name-check').length, 1);
  assert.equal(disconnected.nodes[1].inputs.actual.kind, 'expression');
  const deleted = deleteNodes(graph, ['response']);
  assert.equal(deleted.nodes[0].inputs.actual, undefined);
  const scenario = project.scenarios[0];
  scenario.graph.nodes.reverse();
  const result = await runScenario({ project, scenarioId: scenario.id, mode: 'simulation' });
  assert.equal(result.status, 'passed');
  assert.equal(result.steps[0].nodeId, 'response');
  assert.equal(result.steps[1].inputs.actual, true);
});

test('nested cycles, missing references, unknown operations and excessive nesting are rejected', () => {
  const project = createBlocksProject();
  const check = project.scenarios[0].graph.nodes[1];
  check.inputs.actual = op('not', reference(check.id, 'passed'));
  assert.ok(validateProject(project).some((message) => message.includes('cykl')));
  check.inputs.actual = op('not', reference('absent', 'passed'));
  assert.ok(validateProject(project).some((message) => message.includes('źródłowego')));
  check.inputs.actual = { ...expression('not'), operator: 'eval' } as unknown as Binding;
  assert.ok(validateProject(project).length);
  let nested: Binding = literal(true);
  for (let i = 0; i < 14; i++) nested = op('not', nested);
  check.inputs.actual = nested;
  assert.ok(validateProject(project).length);
});

test('component extraction maps individual expression leaves and preserves the operator tree', async () => {
  const project = createBlocksProject();
  const graph = project.scenarios[0].graph;
  const ids = ['name-check'];
  const inputs = initializeInputs(graph, ids, project);
  const definition = materializeComponent(graph, ids, {
    name: 'Nazwa',
    description: '',
    inputs,
    outputs: initializeOutputs(graph, ids, project),
    action: '',
    actionType: 'any',
    actionContract: '',
  });
  assert.equal(definition.graph.nodes[0].inputs.actual.kind, 'expression');
  assert.equal(definition.inputs.length, 2);
  project.components.push(definition);
  const instance = createNode('component', { x: 400, y: 400 }, definition);
  for (const input of inputs.filter((row) => row.enabled))
    instance.inputs[input.name] = input.original!;
  project.scenarios[0].graph = { nodes: [graph.nodes[0], instance], edges: [] };
  assert.deepEqual(validateProject(project), []);
  const result = await runScenario(
    JSON.parse(
      JSON.stringify({
        project,
        scenarioId: project.scenarios[0].id,
        mode: 'simulation',
      }),
    ),
  );
  assert.equal(result.status, 'passed');
  assert.equal(result.steps[1].children?.[0].actual, true);
});

test('real HTTP receives computed arguments and a nested condition tests its response', async () => {
  const server = createServer(async (req, res) => {
    const chunks: Buffer[] = [];
    for await (const chunk of req) chunks.push(chunk);
    res.setHeader('Content-Type', 'application/json');
    res.end(Buffer.concat(chunks));
  });
  await new Promise<void>((resolve) => server.listen(0, '127.0.0.1', resolve));
  try {
    const project = createEmptyProject();
    project.environment.baseUrl = `http://127.0.0.1:${(server.address() as AddressInfo).port}`;
    const request = createNode('request');
    request.config.method = 'POST';
    request.inputs = {
      amount: op('multiply', literal(12), op('add', literal(2), literal(3))),
      body: literal({ amount: '{{amount}}' }),
    };
    const check = createNode('assert');
    check.inputs = {
      actual: op(
        'and',
        op('equals', op('typeOf', reference(request.id, 'body', 'amount')), literal('number')),
        op('equals', reference(request.id, 'body', 'amount'), literal(60)),
      ),
      expected: literal(true),
    };
    project.scenarios[0].graph = { nodes: [check, request], edges: [] };
    const result = await runScenario({
      project,
      scenarioId: project.scenarios[0].id,
      mode: 'live',
    });
    assert.equal(result.status, 'passed');
    assert.deepEqual(result.steps[0].request?.body, { amount: 60 });
    assert.equal(result.steps[1].actual, true);
  } finally {
    await new Promise<void>((resolve) => server.close(() => resolve()));
  }
});

test('component extraction uses the nested slot type instead of the numeric outer result', async () => {
  const project = createBlocksProject();
  const source = project.scenarios[0].graph.nodes[0];
  const calculation = createNode('formula');
  calculation.inputs = {
    left: op('length', reference(source.id, 'body', 'name')),
    right: literal(2),
  };
  const graph = { nodes: [source, calculation], edges: [] };
  const rows = initializeInputs(graph, [calculation.id], project);
  assert.equal(rows.find((row) => row.original?.kind === 'reference')?.type, 'any');
  const component = materializeComponent(graph, [calculation.id], {
    name: 'Długość razy dwa',
    description: '',
    inputs: rows,
    outputs: initializeOutputs(graph, [calculation.id], project),
    action: '',
    actionType: 'any',
    actionContract: '',
  });
  project.components = [component];
  const instance = createNode('component', { x: 0, y: 0 }, component);
  for (const row of rows.filter((row) => row.enabled)) instance.inputs[row.name] = row.original!;
  project.scenarios[0].graph = { nodes: [source, instance], edges: [] };
  const result = await runScenario(
    JSON.parse(
      JSON.stringify({ project, scenarioId: project.scenarios[0].id, mode: 'simulation' }),
    ),
  );
  assert.equal(result.status, 'passed');
  assert.equal(result.steps[1].outputs.value, 28);
});

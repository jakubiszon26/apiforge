import test from 'node:test';
import assert from 'node:assert/strict';
import { createNode, visualEdges } from '../../../shared/graph';
import { literal } from '../../../shared/model';
import type { Graph, Project } from '../../../shared/model';
import {
  connectGraph,
  copySelection,
  pasteSelection,
  duplicateSelection,
  removeConnections,
} from './operations';

const project: Project = {
  version: 1,
  id: 'test',
  name: 'Test',
  scenarios: [],
  components: [],
  contracts: [],
  environment: { baseUrl: '', variables: {}, secretNames: [] },
};
function fixture() {
  const request = createNode('request'),
    formula = createNode('formula'),
    assertion = createNode('assert');
  const graph: Graph = { nodes: [request, formula, assertion], edges: [] };
  return { request, formula, assertion, graph };
}

test('connections carry reference data; an occupied input cannot be overwritten', () => {
  const { graph, request, formula } = fixture();
  const connected = connectGraph(graph, project, request.id, 'status', formula.id, 'left');
  assert.ok(connected.graph);
  assert.deepEqual(connected.graph.nodes[1].inputs.left, {
    kind: 'reference',
    nodeId: request.id,
    port: 'status',
    path: '',
  });
  assert.equal(connected.graph.edges.length, 0);
  assert.equal(visualEdges(connected.graph).length, 1);
  assert.match(
    connectGraph(connected.graph, project, request.id, 'duration', formula.id, 'left').error!,
    /już źródło/,
  );
  assert.deepEqual(graph.nodes[1].inputs.left, literal(0));
});

test('incompatible input types and mixed data/order cycles are rejected', () => {
  const { graph, request, formula } = fixture();
  assert.match(
    connectGraph(graph, project, request.id, 'headers', formula.id, 'left').error!,
    /niezgodne typy/,
  );
  const forward = connectGraph(graph, project, request.id, 'status', formula.id, 'left').graph!;
  assert.match(
    connectGraph(forward, project, formula.id, '__order', request.id, '__order').error!,
    /cykl/,
  );
  assert.match(
    connectGraph(graph, project, formula.id, 'value', formula.id, 'left').error!,
    /sam ze sobą/,
  );
});

test('duplicate remaps internal links but retains references outside selection', () => {
  const { graph, request, formula, assertion } = fixture();
  const first = connectGraph(graph, project, request.id, 'status', formula.id, 'left').graph!;
  const second = connectGraph(first, project, formula.id, 'value', assertion.id, 'actual').graph!;
  const ordered = connectGraph(
    second,
    project,
    formula.id,
    '__order',
    assertion.id,
    '__order',
  ).graph!;
  const copy = duplicateSelection(ordered, [formula.id, assertion.id]);
  assert.equal(copy.graph.nodes.length, 5);
  const copiedFormula = copy.graph.nodes.find((n) => n.id === copy.ids[0])!,
    copiedAssertion = copy.graph.nodes.find((n) => n.id === copy.ids[1])!;
  assert.equal(copiedFormula.inputs.left.kind, 'reference');
  if (copiedFormula.inputs.left.kind === 'reference')
    assert.equal(copiedFormula.inputs.left.nodeId, request.id);
  if (copiedAssertion.inputs.actual.kind === 'reference')
    assert.equal(copiedAssertion.inputs.actual.nodeId, copiedFormula.id);
  else assert.fail('Missing copied binding');
  assert.ok(
    copy.graph.edges.some((e) => e.source === copiedFormula.id && e.target === copiedAssertion.id),
  );
});

test('removing a visual binding edge clears only that named input', () => {
  const { graph, request, formula } = fixture();
  const first = connectGraph(graph, project, request.id, 'status', formula.id, 'left').graph!;
  const second = connectGraph(first, project, request.id, 'duration', formula.id, 'right').graph!;
  const result = removeConnections(second, [`data:${formula.id}:left`]);
  assert.equal(result.nodes[1].inputs.left, undefined);
  assert.equal(result.nodes[1].inputs.right.kind, 'reference');
});

test('clipboard freezes component configuration and nested arguments; every paste is independent', () => {
  const source = createNode('component');
  source.config = {
    componentId: 'custom',
    revision: 7,
    templateInputs: [{ name: 'filter', type: 'object' }],
  };
  source.inputs = { filter: literal({ active: true, fields: ['name', 'price'] }) };
  const original = structuredClone(source);
  const graph: Graph = { nodes: [source], edges: [] };
  const fragment = copySelection(graph, [source.id]);
  source.config.revision = 8;
  source.inputs.filter = literal({ active: false });
  const first = pasteSelection(graph, fragment);
  const copied = first.graph.nodes.at(-1)!;
  assert.deepEqual(copied.config, original.config);
  assert.deepEqual(copied.inputs, original.inputs);
  copied.config.templateInputs![0].type = 'string';
  const second = pasteSelection(first.graph, fragment, { x: 80, y: 100 });
  const pastedAgain = second.graph.nodes.at(-1)!;
  assert.notEqual(pastedAgain.id, copied.id);
  assert.deepEqual(pastedAgain.config, original.config);
  assert.deepEqual(pastedAgain.position, {
    x: original.position.x + 80,
    y: original.position.y + 100,
  });
  assert.deepEqual(fragment.nodes[0], original);
});

test('pasting a group rewires nested data and IF branches while retaining existing external sources', () => {
  const external = createNode('variable'),
    branch = createNode('if'),
    target = createNode('request');
  target.inputs.body = {
    kind: 'expression',
    operator: 'add',
    inputs: {
      left: { kind: 'reference', nodeId: branch.id, port: 'passed', path: '' },
      right: { kind: 'reference', nodeId: external.id, port: 'value', path: '' },
    },
  };
  const graph: Graph = {
    nodes: [external, branch, target],
    edges: [{ id: 'else', source: branch.id, target: target.id, branch: 'false' }],
  };
  const fragment = copySelection(graph, [branch.id, target.id]);
  // Deleting the originals does not invalidate references within the saved fragment.
  const result = pasteSelection({ nodes: [external], edges: [] }, fragment);
  assert.equal(result.error, undefined);
  const [copiedBranch, copiedTarget] = result.graph.nodes.slice(1);
  assert.deepEqual(result.graph.edges[0], {
    id: result.graph.edges[0].id,
    source: copiedBranch.id,
    target: copiedTarget.id,
    branch: 'false',
  });
  const input = copiedTarget.inputs.body;
  assert.equal(input.kind, 'expression');
  if (input.kind === 'expression') {
    assert.deepEqual(input.inputs.left, {
      kind: 'reference',
      nodeId: copiedBranch.id,
      port: 'passed',
      path: '',
    });
    assert.deepEqual(input.inputs.right, {
      kind: 'reference',
      nodeId: external.id,
      port: 'value',
      path: '',
    });
  }
  assert.match(pasteSelection({ nodes: [], edges: [] }, fragment).error!, /źródła danych/);
  assert.equal(
    pasteSelection({ nodes: [], edges: [] }, fragment, undefined, [external.id]).error,
    undefined,
  );
});

test('pasting Start cannot create an invalid scenario with two triggers', () => {
  const start = createNode('trigger');
  const graph: Graph = { nodes: [start], edges: [] };
  const result = duplicateSelection(graph, [start.id]);
  assert.equal(result.graph, graph);
  assert.deepEqual(result.ids, []);
  assert.match(result.error!, /już Start/);
});

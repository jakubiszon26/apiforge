import assert from 'node:assert/strict';
import test from 'node:test';
import type { ComponentDefinition, Graph, Project, TestNode } from '../../../shared/model';
import { literal, reference } from '../../../shared/model';
import { validateProject } from '../../../shared/graph';
import {
  initializeInputs,
  initializeOutputs,
  materializeComponent,
  validateDraft,
} from './componentDraft';
import type { ComponentDraft } from './componentDraft';

const request = (id: string, inputs: TestNode['inputs'] = { id: literal('123') }): TestNode => ({
  id,
  kind: 'request',
  name: id,
  position: { x: 0, y: 0 },
  inputs,
  config: {
    method: 'GET',
    url: '/records/:id',
    inputPorts: [{ name: 'id', type: 'any', exposed: true, required: true }],
  },
});
const project = (graph: Graph, components: ComponentDefinition[] = []): Project => ({
  version: 1,
  id: 'test-project',
  name: 'Test',
  components,
  contracts: [],
  environment: { baseUrl: 'http://localhost:3000', variables: {}, secretNames: [] },
  scenarios: [
    {
      id: 'scenario',
      name: 'Scenario',
      description: '',
      graph,
      cleanup: { nodes: [], edges: [] },
      parameters: {},
      errorPolicy: 'continue',
    },
  ],
});
function draft(
  graph: Graph,
  ids: string[],
  p = project(graph),
  initial?: ComponentDefinition,
): ComponentDraft {
  return {
    name: 'Reusable request',
    description: '',
    inputs: initializeInputs(graph, ids, p, initial),
    outputs: initializeOutputs(graph, ids, p, initial),
    action: '',
    actionType: 'any',
    actionContract: '',
  };
}

test('extract request with external bound argument and internal dependency, leaving original graph untouched', () => {
  const graph: Graph = {
    nodes: [
      request('source'),
      request('get', { id: reference('source', 'body', 'data.id') }),
      {
        id: 'select',
        kind: 'select',
        name: 'Selected id',
        position: { x: 0, y: 0 },
        config: { path: 'data.id' },
        inputs: { value: reference('get') },
      },
      request('outside', { id: reference('select', 'value') }),
    ],
    edges: [
      { id: 'order', source: 'get', target: 'select' },
      { id: 'outside-order', source: 'select', target: 'outside' },
    ],
  };
  const before = structuredClone(graph);
  const ids = ['get', 'select'];
  const d = draft(graph, ids);
  const arg = d.inputs.find((i) => i.input === 'id')!;
  assert.equal(arg.mandatory, true);
  arg.name = 'recordId';
  const result = materializeComponent(graph, ids, d);
  assert.deepEqual(result.graph.nodes[0].inputs.id, { kind: 'parameter', name: 'recordId' });
  assert.deepEqual(result.graph.nodes[1].inputs.value, reference('get'));
  assert.deepEqual(result.graph.edges, [{ id: 'order', source: 'get', target: 'select' }]);
  assert.deepEqual(graph, before);
  result.graph.nodes[0].config.url = '/changed';
  result.graph.edges[0].id = 'changed';
  assert.deepEqual(graph, before);
});

test('same parameter feeds multiple nodes through one public input; defaults including null survive revision', () => {
  const graph: Graph = {
    nodes: [
      request('one', { id: { kind: 'parameter', name: 'recordId' } }),
      request('two', { id: { kind: 'parameter', name: 'recordId' } }),
    ],
    edges: [],
  };
  const initial: ComponentDefinition = {
    id: 'reusable',
    revision: 3,
    name: 'Read twice',
    description: 'Description',
    inputs: [
      {
        name: 'recordId',
        label: 'Record identifier',
        type: 'any',
        required: true,
        exposed: true,
        defaultValue: null,
      },
    ],
    outputs: [
      {
        name: 'record',
        label: 'Selected record',
        type: 'object',
        binding: reference('two', 'body', 'data.record'),
      },
    ],
    graph,
  };
  const d = draft(graph, ['one', 'two'], project(graph, [initial]), initial);
  assert.equal(d.inputs.filter((i) => i.enabled).length, 1);
  d.inputs[0].name = 'targetId';
  const result = materializeComponent(graph, ['one', 'two'], d, initial);
  assert.equal(result.id, 'reusable');
  assert.equal(result.revision, 3);
  assert.equal(result.inputs[0].defaultValue, null);
  assert.equal(result.inputs[0].label, 'Record identifier');
  assert.equal(result.outputs[0].label, 'Selected record');
  for (const node of result.graph.nodes)
    assert.deepEqual(node.inputs.id, { kind: 'parameter', name: 'targetId' });
  assert.deepEqual(result.outputs[0].binding, reference('two', 'body', 'data.record'));
  assert.deepEqual(initial.graph.nodes[0].inputs.id, { kind: 'parameter', name: 'recordId' });
});

test('direct public-parameter output survives revision and follows argument rename', () => {
  const graph: Graph = { nodes: [request('request')], edges: [] };
  const initial: ComponentDefinition = {
    id: 'reusable',
    revision: 1,
    name: 'Read',
    description: '',
    inputs: [
      { name: 'context', type: 'object', required: false, defaultValue: { source: 'manual' } },
    ],
    outputs: [
      { name: 'context', type: 'object', binding: { kind: 'parameter', name: 'context' } },
      { name: 'body', type: 'any', binding: reference('request') },
    ],
    graph,
  };
  const d = draft(graph, ['request'], project(graph, [initial]), initial);
  d.inputs.find((i) => i.name === 'context')!.name = 'metadata';
  const result = materializeComponent(graph, ['request'], d, initial);
  assert.deepEqual(result.outputs[0].binding, { kind: 'parameter', name: 'metadata' });
  assert.deepEqual(result.inputs.find((i) => i.name === 'metadata')?.defaultValue, {
    source: 'manual',
  });
});

test('duplicate public names and disabled boundary mapping are rejected before saving', () => {
  const graph: Graph = { nodes: [request('one'), request('two')], edges: [] };
  const d = draft(graph, ['one', 'two']);
  d.inputs.filter((i) => i.enabled).forEach((i) => (i.name = 'id'));
  assert.ok(validateDraft(graph, ['one', 'two'], d).some((e) => e.includes('unikalne')));
  assert.throws(() => materializeComponent(graph, ['one', 'two'], d), /unikalne/);
  const boundaryGraph: Graph = {
    nodes: [request('source'), request('target', { id: reference('source') })],
    edges: [],
  };
  const boundaryDraft = draft(boundaryGraph, ['target']);
  boundaryDraft.inputs.find((i) => i.mandatory)!.enabled = false;
  assert.throws(
    () => materializeComponent(boundaryGraph, ['target'], boundaryDraft),
    /publicznymi argumentami/,
  );
});

test('existing output aliases preserve literal, environment and nested field bindings', () => {
  const graph: Graph = { nodes: [request('read')], edges: [] };
  const initial: ComponentDefinition = {
    id: 'component',
    revision: 2,
    name: 'Read',
    description: '',
    inputs: [],
    outputs: [
      { name: 'nested', type: 'string', binding: reference('read', 'body', 'items[0].label') },
      { name: 'literal', type: 'number', binding: literal(0) },
      { name: 'environment', type: 'string', binding: { kind: 'environment', name: 'baseUrl' } },
    ],
    graph,
  };
  const d = draft(graph, ['read'], project(graph, [initial]), initial);
  const result = materializeComponent(graph, ['read'], d, initial);
  assert.deepEqual(
    result.outputs.map((o) => o.binding),
    initial.outputs.map((o) => o.binding),
  );
});

test('root project validation rejects a recursive component definition built from graph nodes', () => {
  const nested: TestNode = {
    id: 'self',
    name: 'Self',
    kind: 'component',
    position: { x: 0, y: 0 },
    inputs: {},
    config: { componentId: 'recursive', revision: 1 },
  };
  const graph: Graph = { nodes: [nested], edges: [] };
  const initial: ComponentDefinition = {
    id: 'recursive',
    revision: 1,
    name: 'Recursive',
    description: '',
    inputs: [],
    outputs: [{ name: 'value', type: 'any', binding: reference('self', 'value') }],
    graph,
  };
  const p = project({ nodes: [], edges: [] }, [initial]);
  const d = draft(graph, ['self'], p, initial);
  const result = materializeComponent(graph, ['self'], d, initial);
  assert.ok(validateProject({ ...p, components: [result] }).some((e) => /rekurencyj/i.test(e)));
});

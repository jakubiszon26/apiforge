import assert from 'node:assert/strict';
import express from 'express';
import { after, before, test } from 'node:test';
import { createServer } from 'node:http';
import type { AddressInfo } from 'node:net';
import { runScenario } from '../api/runner';
import { createApp } from '../api/server';
import { createNode, validateProject } from '../shared/graph';
import { createWorkflowProject } from '../shared/demos';
import { literal, reference } from '../shared/model';
import type { Graph, Project, RunRequest } from '../shared/model';
import { connectGraph, duplicateSelection } from '../app/features/graph/operations';

const received: string[] = [];
let active = 0;
let maxActive = 0;
const app = express();
app.get('/ordered/:name', async (req, res) => {
  active++;
  maxActive = Math.max(maxActive, active);
  received.push(req.params.name);
  await new Promise((resolve) => setTimeout(resolve, 20));
  active--;
  res.json({ name: req.params.name });
});
app.use(createApp());
const server = createServer(app);
let baseUrl = '';
before(async () => {
  await new Promise<void>((resolve) => server.listen(0, '127.0.0.1', resolve));
  baseUrl = `http://127.0.0.1:${(server.address() as AddressInfo).port}`;
});
after(() => new Promise<void>((resolve) => server.close(() => resolve())));
function runRequest(graph: Graph): RunRequest {
  const project: Project = {
    ...createWorkflowProject(),
    environment: { baseUrl, variables: {}, secretNames: [] },
    scenarios: [
      {
        id: 's',
        name: 'flow',
        description: '',
        parameters: {},
        errorPolicy: 'stop',
        graph,
        cleanup: { nodes: [], edges: [] },
      },
    ],
  };
  return { project, scenarioId: 's', mode: 'live' };
}
const block = (kind: Parameters<typeof createNode>[0], id: string) => ({
  ...createNode(kind),
  id,
  name: id,
});
const http = (id: string) => ({
  ...block('request', id),
  config: { method: 'GET', url: `/ordered/${id}` },
});
const edge = (source: string, target: string) => ({ id: `${source}-${target}`, source, target });

test('explicit order awaits HTTP completion even with reversed node storage; disconnected requests never fire', async () => {
  received.length = 0;
  maxActive = 0;
  const graph: Graph = {
    nodes: [
      http('add'),
      http('unconnected'),
      http('cart'),
      http('prices'),
      block('trigger', 'start'),
    ],
    edges: [edge('start', 'prices'), edge('prices', 'cart'), edge('cart', 'add')],
  };
  const run = await runScenario(runRequest(graph));
  assert.equal(run.status, 'passed');
  assert.deepEqual(received, ['prices', 'cart', 'add']);
  assert.equal(maxActive, 1);
  assert.equal(run.steps.find((s) => s.nodeId === 'unconnected')?.status, 'skipped');
});

for (const passed of [true, false])
  test(`IF ${passed} activates only its selected branch and merges alternatives`, async () => {
    received.length = 0;
    const branch = block('if', 'if');
    branch.inputs = { actual: literal(passed), expected: literal(true) };
    const graph: Graph = {
      nodes: [
        http('join'),
        http('yes-next'),
        http('no-next'),
        http('yes'),
        http('no'),
        branch,
        block('trigger', 'start'),
      ],
      edges: [
        edge('start', 'if'),
        { ...edge('if', 'yes'), branch: 'true' },
        { ...edge('if', 'no'), branch: 'false' },
        edge('yes', 'yes-next'),
        edge('no', 'no-next'),
        edge('yes-next', 'join'),
        edge('no-next', 'join'),
      ],
    };
    const run = await runScenario(runRequest(graph));
    assert.equal(run.status, 'passed');
    assert.deepEqual(received, passed ? ['yes', 'yes-next', 'join'] : ['no', 'no-next', 'join']);
    assert.equal(run.steps.find((s) => s.nodeId === 'if')?.status, 'passed');
    assert.equal(run.steps.find((s) => s.nodeId === 'if')?.outputs.passed, passed);
  });

test('references to an inactive branch cannot cause HTTP side effects', async () => {
  received.length = 0;
  const branch = block('if', 'if');
  branch.inputs = { actual: literal(false), expected: literal(true) };
  const dependent = http('dependent');
  dependent.inputs.body = reference('yes', 'body');
  const run = await runScenario(
    runRequest({
      nodes: [block('trigger', 'start'), branch, http('yes'), dependent],
      edges: [edge('start', 'if'), { ...edge('if', 'yes'), branch: 'true' }],
    }),
  );
  assert.equal(run.status, 'passed');
  assert.deepEqual(received, []);
  assert.equal(run.steps.find((s) => s.nodeId === 'dependent')?.status, 'skipped');
});

test('a failed active request blocks its successors', async () => {
  received.length = 0;
  const broken = http('broken');
  broken.config.url = 'http://[';
  const run = await runScenario(
    runRequest({
      nodes: [block('trigger', 'start'), broken, http('after')],
      edges: [edge('start', 'broken'), edge('broken', 'after')],
    }),
  );
  assert.equal(run.status, 'failed');
  assert.deepEqual(received, []);
});

test('cart workflow uses fresh response values, then cleans up the created resource', async () => {
  const project = createWorkflowProject();
  project.environment.baseUrl = `${baseUrl}/demo/shop`;
  assert.deepEqual(validateProject(project), []);
  const run = await runScenario({ project, scenarioId: 'workflow-cart', mode: 'live' });
  assert.equal(run.status, 'passed');
  const requests = run.steps.filter((s) => s.request);
  assert.deepEqual(
    requests.map((s) => s.nodeId),
    ['flow-prices', 'flow-cart', 'flow-add', 'flow-cleanup'],
  );
  assert.deepEqual(run.steps.find((s) => s.nodeId === 'flow-add')?.inputs.body, {
    productId: 123,
    quantity: 2,
  });
  assert.equal(run.steps.find((s) => s.nodeId === 'flow-check')?.actual, 160);
  const id = (run.steps.find((s) => s.nodeId === 'flow-cart')!.outputs.body as { id: string }).id;
  assert.equal((await fetch(`${baseUrl}/demo/shop/carts/${id}`)).status, 404);
});

test('HTTP 404 selects ELSE and sends no create/add/cleanup request', async () => {
  const project = createWorkflowProject();
  project.environment.baseUrl = `${baseUrl}/demo/shop`;
  project.scenarios[0].graph.nodes.find((n) => n.id === 'flow-prices')!.config.url =
    '/products/999';
  const run = await runScenario({ project, scenarioId: 'workflow-cart', mode: 'live' });
  assert.equal(run.status, 'failed'); // The ELSE branch deliberately reports the unexpected HTTP status.
  assert.deepEqual(
    run.steps.filter((s) => s.request).map((s) => s.nodeId),
    ['flow-prices'],
  );
  assert.equal(run.steps.find((s) => s.nodeId === 'flow-else')?.status, 'failed');
  assert.equal(run.steps.find((s) => s.nodeId === 'flow-cart')?.status, 'skipped');
});

test('control connections preserve branch metadata through duplication and reject invalid ports/cycles', () => {
  const branch = block('if', 'if'),
    next = http('next');
  const graph: Graph = { nodes: [block('trigger', 'start'), branch, next], edges: [] };
  const project = runRequest(graph).project;
  assert.ok(connectGraph(graph, project, 'if', '__order', 'next', '__order').error);
  assert.ok(connectGraph(graph, project, 'next', '__order', 'start', '__order').error);
  const connected = connectGraph(graph, project, 'if', '__false', 'next', '__order').graph!;
  assert.equal(connected.edges[0].branch, 'false');
  assert.equal(duplicateSelection(connected, ['if', 'next']).graph.edges.at(-1)?.branch, 'false');
  assert.ok(connectGraph(connected, project, 'next', '__order', 'if', '__order').error);
  const invalid = structuredClone(project);
  invalid.scenarios[0].graph.edges.push({ ...edge('next', 'if'), branch: 'true' });
  assert.ok(validateProject(invalid).some((message) => message.includes('IF')));
});

test('merging alternatives does not hide an error on an active predecessor', async () => {
  received.length = 0;
  const broken = http('broken');
  broken.config.url = 'http://[';
  const request = runRequest({
    nodes: [block('trigger', 'start'), broken, http('good'), http('join')],
    edges: [
      edge('start', 'broken'),
      edge('start', 'good'),
      edge('broken', 'join'),
      edge('good', 'join'),
    ],
  });
  request.project.scenarios[0].errorPolicy = 'continue';
  const run = await runScenario(request);
  assert.equal(run.status, 'failed');
  assert.deepEqual(received, ['good']);
  assert.equal(run.steps.find((step) => step.nodeId === 'join')?.status, 'blocked');
});

test('single-step diagnosis bypasses only its own gate; nested IF still selects one branch', async () => {
  const project = createWorkflowProject();
  project.environment.baseUrl = `${baseUrl}/demo/shop`;
  const graph = project.scenarios[0].graph;
  const componentGraph: Graph = {
    nodes: [
      block('trigger', 'inner-start'),
      block('if', 'inner-if'),
      http('inner-yes'),
      http('inner-no'),
    ],
    edges: [
      edge('inner-start', 'inner-if'),
      { ...edge('inner-if', 'inner-yes'), branch: 'true' },
      { ...edge('inner-if', 'inner-no'), branch: 'false' },
    ],
  };
  for (const node of componentGraph.nodes.filter((node) => node.kind === 'request'))
    node.config.url = `${baseUrl}${node.config.url}`;
  componentGraph.nodes[1].inputs = { actual: literal(false), expected: literal(true) };
  project.components = [
    {
      id: 'c',
      revision: 1,
      name: 'c',
      description: '',
      inputs: [],
      outputs: [],
      graph: componentGraph,
    },
  ];
  const component = block('component', 'isolated-component');
  component.config = { componentId: 'c', revision: 1 };
  graph.nodes.push(component);
  // Its parent is disconnected from Start. The explicit diagnostic action may still run it.
  received.length = 0;
  const run = await runScenario({
    project,
    scenarioId: 'workflow-cart',
    mode: 'live',
    singleNodeId: component.id,
  });
  const children = run.steps.find((step) => step.nodeId === component.id)?.children;
  assert.equal(children?.find((step) => step.nodeId === 'inner-yes')?.status, 'skipped');
  assert.equal(children?.find((step) => step.nodeId === 'inner-no')?.status, 'passed');
  assert.ok(children?.find((step) => step.nodeId === 'inner-no')?.request);
});

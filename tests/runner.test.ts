import assert from 'node:assert/strict';
import { after, before, test } from 'node:test';
import { createServer } from 'node:http';
import type { AddressInfo } from 'node:net';
import { runScenario } from '../api/runner';
import { validateRunRequest } from '../api/runner/validation';
import { createNode } from '../shared/graph';
import { literal, reference } from '../shared/model';
import type { Project, RunRequest, TestNode } from '../shared/model';

let baseUrl = '';
let cleaned = 0;
let creates = 0;
const server = createServer(async (req, res) => {
  if (req.url?.startsWith('/slow')) {
    setTimeout(() => {
      res.end('{}');
    }, 150);
    return;
  }
  if (req.url === '/large') {
    res.setHeader('Content-Length', 3 * 1024 * 1024);
    res.end();
    return;
  }
  if (req.url === '/nonfinite') {
    res.end('{"amount":1e999}');
    return;
  }
  if (req.method === 'DELETE') {
    cleaned++;
    res.end('{}');
    return;
  }
  if (req.method === 'POST') {
    creates++;
    const buffers: Buffer[] = [];
    for await (const chunk of req) buffers.push(chunk);
    res.setHeader('Content-Type', 'application/json');
    res.end(
      JSON.stringify({
        id: 'new',
        body: JSON.parse(Buffer.concat(buffers).toString()),
        authorization: req.headers.authorization,
      }),
    );
    return;
  }
  if (req.url === '/bad') {
    res.statusCode = 422;
    res.end('{"error":"invalid"}');
    return;
  }
  res.setHeader('Content-Type', 'application/json');
  res.end(JSON.stringify({ id: 'item', price: 100, empty: null, token: 'response-secret' }));
});
before(async () => {
  await new Promise<void>((resolve, reject) => {
    server.once('error', reject);
    server.listen(0, '127.0.0.1', resolve);
  });
  baseUrl = `http://127.0.0.1:${(server.address() as AddressInfo).port}`;
});
after(() => new Promise<void>((resolve) => server.close(() => resolve())));
function request(nodes: TestNode[], cleanup: TestNode[] = []): RunRequest {
  const project: Project = {
    version: 1,
    id: 'p',
    name: 'test',
    scenarios: [
      {
        id: 's',
        name: 'test',
        description: '',
        parameters: {},
        errorPolicy: 'continue',
        graph: { nodes, edges: [] },
        cleanup: { nodes: cleanup, edges: [] },
      },
    ],
    components: [],
    contracts: [],
    environment: { baseUrl, variables: {}, secretNames: [] },
  };
  return { project, scenarioId: 's', mode: 'live' };
}
function node(kind: TestNode['kind'], id: string): TestNode {
  const n = createNode(kind);
  n.id = id;
  n.name = id;
  return n;
}
function http(id: string, url = '/', method = 'GET'): TestNode {
  const n = node('request', id);
  n.config = {
    url,
    method,
    sampleResponse: { id: 'item', price: 100, empty: null },
    inputPorts: [],
  };
  n.inputs = {};
  return n;
}
function check(id: string, source: string, path: string, expected: any): TestNode {
  const n = node('assert', id);
  n.inputs = { actual: reference(source, 'body', path), expected: literal(expected) };
  return n;
}

test('real HTTP, status 4xx as data, typed body interpolation and cleanup', async () => {
  const first = http('first');
  const post = http('post', '/items', 'POST');
  post.inputs = {
    body: literal({ amount: '{{amount}}', id: '{{id}}' }),
    amount: literal(20),
    id: reference('first', 'body', 'id'),
  };
  const bad = http('bad', '/bad');
  const verify = check('verify', 'post', 'body.amount', 20);
  const status = check('status', 'bad', '', 422);
  status.inputs.actual = reference('bad', 'status', '');
  const cleanup = http('cleanup', '/items/:id', 'DELETE');
  cleanup.inputs.id = reference('post', 'body', 'id');
  const run = await runScenario(request([verify, status, post, first, bad], [cleanup]));
  assert.equal(run.status, 'passed');
  assert.equal(run.steps.find((s) => s.nodeId === 'post')!.outputs.status, 200);
  assert.equal(run.steps.at(-1)!.phase, 'cleanup');
  assert.ok(cleaned > 0);
  assert.ok(creates > 0);
});
test('failed assertion preserves data; condition blocks descendants, independent branch continues', async () => {
  const first = http('first');
  const failed = check('failed', 'first', 'price', 80);
  const next = node('select', 'next');
  next.inputs.value = reference('failed', 'value', '');
  const gate = node('condition', 'gate');
  gate.inputs = { actual: literal(false), expected: literal(true) };
  const blocked = node('build', 'blocked');
  blocked.inputs.field = reference('gate', 'passed', '');
  const run = await runScenario(request([first, failed, next, gate, blocked]));
  assert.equal(run.status, 'failed');
  assert.equal(run.steps[1].status, 'failed');
  assert.equal(run.steps[2].outputs.value, 100);
  assert.equal(run.steps[3].status, 'blocked');
  assert.equal(run.steps[4].status, 'blocked');
});
test('missing vs null, zero and absent select output; formulas reject null', async () => {
  const first = http('first');
  const missing = check('missing', 'first', 'missing', null);
  missing.config.predicate = 'exists';
  const nullable = check('null', 'first', 'empty', null);
  const formula = node('formula', 'formula');
  formula.inputs = { left: reference('first', 'body', 'empty'), right: literal(2) };
  const select = node('select', 'select');
  select.inputs.value = reference('first', 'body', '');
  select.config.path = 'absent';
  const absent = node('assert', 'absent');
  absent.inputs.actual = reference('select', 'value', '');
  absent.config.predicate = 'exists';
  const run = await runScenario(request([first, missing, nullable, formula, select, absent]));
  assert.deepEqual(
    run.steps.map((s) => s.status),
    ['passed', 'failed', 'passed', 'error', 'passed', 'failed'],
  );
  assert.deepEqual(run.steps[4].outputs, {});
});
test('stop policy skips remaining main but still cleans up; timeout and response limit are errors', async () => {
  const slow = http('slow', '/slow');
  slow.config.timeoutMs = 15;
  const later = http('later');
  const cleanup = http('cleanup', '/items/known', 'DELETE');
  const req = request([slow, later], [cleanup]);
  req.project.scenarios[0].errorPolicy = 'stop';
  const run = await runScenario(req);
  assert.deepEqual(
    run.steps.map((s) => s.status),
    ['error', 'skipped', 'passed'],
  );
  assert.match(run.steps[0].message!, /czas/);
  const large = await runScenario(request([http('large', '/large')]));
  assert.equal(large.steps[0].status, 'error');
  assert.match(large.steps[0].message!, /2 MiB/);
});
test('cancel aborts request and cleanup uses independent signal', async () => {
  const controller = new AbortController();
  const pending = runScenario(
    request([http('slow', '/slow')], [http('cleanup', '/items/known', 'DELETE')]),
    { signal: controller.signal },
  );
  setTimeout(() => controller.abort(), 10);
  const run = await pending;
  assert.equal(run.status, 'cancelled');
  assert.equal(run.steps[0].status, 'cancelled');
  assert.equal(run.steps[1].status, 'passed');
});
test('redacts secrets from snapshots, requests, responses, callbacks and encoded URLs', async () => {
  const post = http('post', '/items?key={{key}}', 'POST');
  post.config.headers = { Authorization: 'Bearer {{key}}' };
  post.inputs = { key: { kind: 'environment', name: 'API_KEY' }, body: literal({}) };
  const req = request([post]);
  req.secrets = { API_KEY: 'my+session+secret' };
  req.project.environment.secretNames = ['API_KEY'];
  const updates: string[] = [];
  const run = await runScenario(req, { onUpdate: (r) => updates.push(JSON.stringify(r)) });
  const all = JSON.stringify(run) + updates.join('');
  assert.equal(run.status, 'passed');
  assert.ok(!all.includes('my+session+secret'));
  assert.ok(!all.includes('my%2Bsession%2Bsecret'));
  assert.ok(all.includes('[REDACTED]'));
  const second = await runScenario(request([http('read')]));
  assert.ok(!JSON.stringify(second).includes('response-secret'));
  const query = http('query', '/?api_key=hardcoded+query+secret');
  const queryUpdates: string[] = [];
  await runScenario(request([query]), { onUpdate: (r) => queryUpdates.push(JSON.stringify(r)) });
  assert.ok(
    queryUpdates.every(
      (r) => !r.includes('hardcoded+query+secret') && !r.includes('hardcoded query secret'),
    ),
  );
});
test('pinned component revisions, nested failures and simulation use same semantics', async () => {
  const internal = node('formula', 'calc');
  internal.config.operation = 'multiply';
  internal.inputs = { left: { kind: 'parameter', name: 'amount' }, right: literal(2) };
  const outer = node('component', 'outer');
  outer.config = { componentId: 'c', revision: 1 };
  outer.inputs.amount = literal(3);
  const req = request([outer]);
  req.project.components = [
    {
      id: 'c',
      revision: 1,
      name: 'Double',
      description: '',
      inputs: [{ name: 'amount', type: 'number', required: true }],
      outputs: [{ name: 'result', type: 'number', binding: reference('calc', 'value', '') }],
      graph: { nodes: [internal], edges: [] },
    },
  ];
  const other = structuredClone(req.project.components[0]);
  other.revision = 2;
  other.graph.nodes[0].inputs.right = literal(10);
  req.project.components.push(other);
  for (const mode of ['simulation', 'live'] as const) {
    req.mode = mode;
    const run = await runScenario(req);
    assert.equal(run.status, 'passed');
    assert.equal(run.steps[0].outputs.result, 6);
    assert.equal(run.steps[0].children?.[0].nodeId, 'calc');
  }
  req.project.components[0].graph.nodes[0].inputs.left = literal(null);
  const run = await runScenario(req);
  assert.equal(run.steps[0].status, 'error');
  assert.equal(run.steps[0].children?.[0].status, 'error');
});
test('partial runs use only ancestors and single-step historical bindings are diagnostic', async () => {
  const first = http('first');
  const verify = check('verify', 'first', 'price', 100);
  const unrelated = http('unrelated');
  const req = request([first, verify, unrelated]);
  req.targetNodeId = 'verify';
  const previous = await runScenario(req);
  assert.equal(previous.diagnostic, true);
  assert.deepEqual(
    previous.steps.map((s) => s.nodeId),
    ['first', 'verify'],
  );
  delete req.targetNodeId;
  req.singleNodeId = 'verify';
  req.previousRun = previous;
  const single = await runScenario(req);
  assert.equal(single.steps.length, 1);
  assert.equal(single.status, 'passed');
  assert.match(single.error!, /historyczne/);
});
test('rejects malformed input, missing revision, graph cycles and component recursion', () => {
  assert.throws(() => validateRunRequest({ project: {} }));
  const a = node('select', 'a');
  a.inputs.value = reference('a', 'value', '');
  assert.throws(() => validateRunRequest(request([a])), /cykl/);
  const c = node('component', 'c');
  c.config = { componentId: 'c', revision: 1 };
  const req = request([c]);
  assert.throws(() => validateRunRequest(req), /rewizji/);
  req.project.components = [
    {
      id: 'c',
      revision: 1,
      name: 'recursive',
      description: '',
      inputs: [],
      outputs: [],
      graph: { nodes: [c], edges: [] },
    },
  ];
  assert.throws(() => validateRunRequest(req), /Rekurencyjna/);
});
test('cleanup with missing created ID is skipped; cleanup errors do not overwrite failed assertion', async () => {
  const read = http('read');
  const bad = check('bad', 'read', 'price', 20);
  const cleanup = http('cleanup', '/items/:id', 'DELETE');
  cleanup.inputs.id = reference('read', 'body', 'nonexistent');
  const run = await runScenario(request([read, bad], [cleanup]));
  assert.equal(run.status, 'failed');
  assert.equal(run.steps[1].status, 'failed');
  assert.equal(run.steps[2].status, 'skipped');
  const slow = http('cleanup', '/slow');
  slow.config.timeoutMs = 10;
  const failed = await runScenario(request([read, bad], [slow]));
  assert.equal(failed.steps[1].status, 'failed');
  assert.equal(failed.steps[2].status, 'error');
  assert.equal(failed.steps[2].phase, 'cleanup');
});
test('cleanup uses exported creation ID even if component later errors; main dependencies stay blocked', async () => {
  const create = http('create', '/items', 'POST');
  create.inputs.body = literal({});
  const broken = node('formula', 'broken');
  broken.config.operation = 'divide';
  broken.inputs = { left: literal(1), right: literal(0) };
  const component = node('component', 'component');
  component.config = { componentId: 'c', revision: 1 };
  const next = node('select', 'next');
  next.inputs.value = reference('component', 'id', '');
  const cleanup = http('cleanup', '/items/:id', 'DELETE');
  cleanup.inputs.id = reference('component', 'id', '');
  const req = request([component, next], [cleanup]);
  req.project.components = [
    {
      id: 'c',
      revision: 1,
      name: 'create then fail',
      description: '',
      inputs: [],
      outputs: [{ name: 'id', type: 'string', binding: reference('create', 'body', 'id') }],
      graph: { nodes: [create, broken], edges: [] },
    },
  ];
  const run = await runScenario(req);
  assert.equal(run.steps[0].status, 'error');
  assert.equal(run.steps[0].outputs.id, 'new');
  assert.equal(run.steps[1].status, 'blocked');
  assert.equal(run.steps[2].status, 'passed');
  assert.equal(run.steps[2].outputs.status, 200);
});
test('nonfinite numbers cannot silently become null in reports or arithmetic', async () => {
  const nonfinite = await runScenario(request([http('nonfinite', '/nonfinite')]));
  assert.equal(nonfinite.steps[0].status, 'error');
  assert.match(nonfinite.steps[0].message!, /skończonym/);
  const overflow = node('formula', 'overflow');
  overflow.config.operation = 'multiply';
  overflow.inputs = { left: literal(Number.MAX_VALUE), right: literal(2) };
  const run = await runScenario(request([overflow]));
  assert.equal(run.steps[0].status, 'error');
  assert.deepEqual(run.steps[0].outputs, {});
});

import assert from 'node:assert/strict';
import { after, before, test } from 'node:test';
import { createServer, request as httpRequest } from 'node:http';
import type { AddressInfo } from 'node:net';
import { createApp } from '../api/server';
import type { RunRequest } from '../shared/model';
import { createShopProject, createTicketsProject } from '../shared/demos';
import { runScenario } from '../api/runner';
const server = createServer(createApp());
let url = '';
before(async () => {
  await new Promise<void>((resolve, reject) => {
    server.once('error', reject);
    server.listen(0, '127.0.0.1', resolve);
  });
  url = `http://127.0.0.1:${(server.address() as AddressInfo).port}`;
});
after(() => new Promise<void>((resolve) => server.close(() => resolve())));
const headers = { Host: '127.0.0.1:4318', 'Content-Type': 'application/json' };
// Node fetch rewrites Host; use native HTTP so the guard is tested explicitly.
function fetch(address: string, init: RequestInit = {}): Promise<Response> {
  return new Promise((resolve, reject) => {
    const req = httpRequest(
      address,
      { method: init.method ?? 'GET', headers: init.headers as Record<string, string> },
      (res) => {
        const chunks: Buffer[] = [];
        res.on('data', (chunk) => chunks.push(chunk));
        res.on('end', () =>
          resolve(
            new Response(res.statusCode === 204 ? null : Buffer.concat(chunks), {
              status: res.statusCode,
              headers: res.headers as Record<string, string>,
            }),
          ),
        );
      },
    );
    req.on('error', reject);
    req.end(init.body);
  });
}
test('local API rejects foreign origin, hostile host and non-JSON mutations', async () => {
  assert.equal(
    (await fetch(`${url}/api/health`, { headers: { Host: 'evil.example' } })).status,
    403,
  );
  assert.equal(
    (
      await fetch(`${url}/api/runs`, {
        method: 'POST',
        headers: { ...headers, Origin: 'https://attacker.example' },
        body: '{}',
      })
    ).status,
    403,
  );
  assert.equal(
    (
      await fetch(`${url}/api/runs`, {
        method: 'POST',
        headers: { Host: '127.0.0.1:4318' },
        body: '{}',
      })
    ).status,
    415,
  );
  const preflight = await fetch(`${url}/api/runs`, {
    method: 'OPTIONS',
    headers: { ...headers, Origin: 'http://localhost:5173' },
  });
  assert.equal(preflight.status, 204);
  assert.equal(preflight.headers.get('access-control-allow-origin'), 'http://localhost:5173');
  assert.equal(
    (await fetch(`${url}/api/runs`, { method: 'POST', headers, body: '{}' })).status,
    400,
  );
});
test('POST returns run ID, GET report and cancellation handles missing IDs', async () => {
  const input: RunRequest = {
    mode: 'simulation',
    scenarioId: 's',
    project: {
      version: 1,
      id: 'p',
      name: 'test',
      contracts: [],
      components: [],
      environment: { baseUrl: 'http://127.0.0.1:4318', variables: {}, secretNames: [] },
      scenarios: [
        {
          id: 's',
          name: 's',
          description: '',
          parameters: {},
          errorPolicy: 'continue',
          graph: { nodes: [], edges: [] },
          cleanup: { nodes: [], edges: [] },
        },
      ],
    },
  };
  const start = await fetch(`${url}/api/runs`, {
    method: 'POST',
    headers,
    body: JSON.stringify(input),
  });
  assert.equal(start.status, 202);
  const { id } = await start.json();
  assert.equal(typeof id, 'string');
  const read = await fetch(`${url}/api/runs/${id}`, { headers });
  assert.equal(read.status, 200);
  const result = await read.json();
  assert.equal(result.status, 'passed');
  assert.equal(result.mode, 'simulation');
  assert.equal(
    (await fetch(`${url}/api/runs/missing/cancel`, { method: 'POST', headers, body: '{}' })).status,
    404,
  );
});
test('actual demo projects preserve API prefixes and cart cleanup really removes resource', async () => {
  for (const project of [createShopProject(), createTicketsProject()]) {
    project.environment.baseUrl = project.environment.baseUrl.replace('http://127.0.0.1:4318', url);
    const run = await runScenario({ project, scenarioId: project.scenarios[0].id, mode: 'live' });
    assert.equal(run.status, 'passed', JSON.stringify(run.steps));
    if (project.id === 'shop') {
      const cart = run.steps.find((s) => s.nodeId === 'cart')!.outputs.cart as { id: string };
      assert.equal(run.steps.find((s) => s.nodeId === 'delete-cart')!.outputs.status, 200);
      const deleted = await fetch(`${project.environment.baseUrl}/carts/${cart.id}`, { headers });
      assert.equal(deleted.status, 404);
    }
  }
});

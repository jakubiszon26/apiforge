import test from 'node:test';
import assert from 'node:assert/strict';
import { decodeJsonResponse } from '../app/features/workspace/http';
import { createShopProject } from '../shared/demos';
import { parseProject } from '../app/features/workspace/storage';

test('runner response parser reports HTML instead of leaking a JSON syntax exception', async () => {
  await assert.rejects(
    decodeJsonResponse(
      new Response('<!doctype html><title>Other app</title>', {
        status: 404,
        headers: { 'Content-Type': 'text/html' },
      }),
    ),
    /inna aplikacja/,
  );
});
test('empty, malformed and structured error responses have distinct actionable messages', async () => {
  await assert.rejects(decodeJsonResponse(new Response('', { status: 502 })), /pustą odpowiedź/);
  await assert.rejects(
    decodeJsonResponse(
      new Response('{broken', { headers: { 'Content-Type': 'application/json' } }),
    ),
    /niepoprawny JSON/,
  );
  await assert.rejects(
    decodeJsonResponse(Response.json({ error: 'Brak definicji komponentu' }, { status: 400 })),
    /Brak definicji komponentu/,
  );
  assert.deepEqual(await decodeJsonResponse(Response.json({ id: 'abc' })), { id: 'abc' });
});
test('old demo addresses migrate without changing custom API addresses or graph', () => {
  const p = createShopProject();
  p.environment.baseUrl = 'http://127.0.0.1:3000/demo/shop';
  const migrated = parseProject(JSON.stringify(p));
  assert.equal(migrated.environment.baseUrl, 'http://127.0.0.1:4318/demo/shop');
  assert.deepEqual(migrated.scenarios, p.scenarios);
  p.environment.baseUrl = 'http://localhost:3000/my-api';
  assert.equal(parseProject(JSON.stringify(p)).environment.baseUrl, p.environment.baseUrl);
});

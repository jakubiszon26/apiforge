import test from 'node:test';
import assert from 'node:assert/strict';
import { redactReport } from './redact';

test('exports redact nested conventional secrets, credentials and query tokens without mutating report', () => {
  const report = {
    request: {
      url: 'https://alice:password@example.test/path?access_token=secret&limit=3',
      headers: { Authorization: 'Bearer abc', 'X-API-Key': 'key' },
      body: { user: { password: 'pw' }, count: 0 },
    },
    outputs: { body: { access_token: 'response-secret', rows: [{ token: 'nested-secret' }] } },
  };
  const redacted = redactReport(report);
  const serialized = JSON.stringify(redacted);
  for (const value of ['Bearer abc', 'response-secret', 'nested-secret', 'alice', 'password@'])
    assert.ok(!serialized.includes(value));
  assert.equal(redacted.request.headers.Authorization, '[REDACTED]');
  assert.equal(redacted.request.body.count, 0);
  assert.equal(new URL(redacted.request.url).searchParams.get('access_token'), '[REDACTED]');
  assert.equal(new URL(redacted.request.url).searchParams.get('limit'), '3');
  assert.equal(report.outputs.body.access_token, 'response-secret');
});

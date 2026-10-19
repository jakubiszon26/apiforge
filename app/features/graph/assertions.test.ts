import test from 'node:test';
import assert from 'node:assert/strict';
import { createFieldAssertion } from './assertions';
import { createNode, inputPorts } from '../../../shared/graph';
import { literal, reference } from '../../../shared/model';
import type { Json } from '../../../shared/model';
import { createEmptyProject } from '../../../shared/demos';
import { runScenario } from '../../../api/runner';

test('new requests have no domain-specific arguments or response fields', () => {
  const node = createNode('request');
  assert.deepEqual(node.inputs, {});
  assert.deepEqual(
    inputPorts(node, createEmptyProject()).map((port) => port.name),
    ['body'],
  );
  assert.equal(node.config.url, '/endpoint');
  assert.equal(node.config.sampleResponse, undefined);
});

test('one comparison accepts value and type arguments and reports mismatches', async () => {
  const cases: [Json, string][] = [
    ['hello', 'string'],
    [0, 'number'],
    [false, 'boolean'],
    [{}, 'object'],
    [[], 'array'],
    [null, 'null'],
  ];
  for (const [value, type] of cases) {
    const project = createEmptyProject();
    project.environment.baseUrl = 'http://example.test';
    const source = createNode('request');
    source.config.sampleResponse = { arbitrary: value };
    const check = createFieldAssertion(source, 'body', 'arbitrary', value);
    assert.equal(check.kind, 'assert');
    assert.equal(check.config.predicate, 'equals');
    assert.deepEqual(check.inputs.expected, literal(value));
    check.config.predicate = 'type';
    check.inputs.expected = literal(type);
    assert.deepEqual(check.inputs.actual, reference(source.id, 'body', 'arbitrary'));
    assert.deepEqual(check.inputs.expected, literal(type));
    project.scenarios[0].graph.nodes = [source, check];
    const result = await runScenario({
      project,
      scenarioId: project.scenarios[0].id,
      mode: 'simulation',
    });
    assert.equal(result.status, 'passed');
    check.inputs.expected = literal(type === 'string' ? 'number' : 'string');
    const failed = await runScenario({
      project,
      scenarioId: project.scenarios[0].id,
      mode: 'simulation',
    });
    assert.equal(failed.steps[1].status, 'failed');
  }
});

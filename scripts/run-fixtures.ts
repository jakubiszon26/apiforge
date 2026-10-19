/**
 * Expected run reports of the TypeScript reference runner, used by the Rust port as
 * a differential corpus. Every run uses simulation mode (sample responses, no
 * network) and a seeded Math.random, so regeneration is deterministic.
 *
 * Reports are normalised: run IDs, timestamps and durations are dropped, and so are
 * messages, which the reference writes in Polish. Statuses, phases, inputs, outputs,
 * requests, compared values and component children are kept.
 */
import { runScenario } from '../api/runner';
import {
  createBlocksProject,
  createShopProject,
  createTicketsProject,
  createWorkflowProject,
} from '../shared/demos';
import { createNode, validateProject } from '../shared/graph';
import { literal, reference } from '../shared/model';
import type {
  Binding,
  ComponentDefinition,
  Graph,
  Json,
  NodeKind,
  Project,
  RunRequest,
  RunResult,
  StepResult,
  TestNode,
} from '../shared/model';

type Normalize = (project: Project, slug: string) => Project;

interface RunCase {
  slug: string;
  description: string;
  request: RunRequest;
}

/** Seed of the Math.random replacement; the Rust replay uses the same generator. */
const RANDOM_SEED = 42;
const HISTORY_TIME = '2026-01-01T00:00:00.000Z';

/** Linear congruential generator (Numerical Recipes constants), 32-bit state. */
function createRng(seed: number): () => number {
  let state = seed >>> 0;
  return () => {
    state = (Math.imul(state, 1664525) + 1013904223) >>> 0;
    return state / 2 ** 32;
  };
}

function node(kind: NodeKind, id: string, patch: Partial<TestNode> = {}): TestNode {
  const created = createNode(kind, { x: 0, y: 0 });
  return {
    ...created,
    id,
    name: id,
    ...patch,
    config: { ...created.config, ...(patch.config ?? {}) },
  };
}

function request(
  id: string,
  url: string,
  sampleResponse: Json | undefined,
  options: { method?: string; inputs?: Record<string, Binding>; sampleStatus?: number } = {},
): TestNode {
  const created = node('request', id, { inputs: options.inputs ?? {} });
  created.config = {
    ...created.config,
    method: options.method ?? 'GET',
    url,
    ...(sampleResponse !== undefined ? { sampleResponse } : {}),
    ...(options.sampleStatus !== undefined ? { sampleStatus: options.sampleStatus } : {}),
  };
  return created;
}

function check(
  kind: 'assert' | 'condition' | 'if',
  id: string,
  actual: Binding,
  expected: Binding,
  config: TestNode['config'] = {},
): TestNode {
  return node(kind, id, {
    inputs: { actual, expected },
    config: { predicate: 'equals', ...config },
  });
}

const edge = (source: string, target: string, branch?: 'true' | 'false') => ({
  id: `${source}->${target}`,
  source,
  target,
  ...(branch ? { branch } : {}),
});

function project(
  id: string,
  graph: Graph,
  options: {
    cleanup?: Graph;
    components?: ComponentDefinition[];
    errorPolicy?: 'stop' | 'continue';
    parameters?: Record<string, Json>;
    environment?: Project['environment'];
  } = {},
): Project {
  return {
    version: 1,
    id,
    name: id,
    components: options.components ?? [],
    contracts: [],
    environment: options.environment ?? {
      baseUrl: 'https://api.example.test/v1/',
      variables: { region: 'eu' },
      secretNames: [],
    },
    scenarios: [
      {
        id: 'main',
        name: 'main',
        description: '',
        parameters: options.parameters ?? {},
        errorPolicy: options.errorPolicy ?? 'continue',
        graph,
        cleanup: options.cleanup ?? { nodes: [], edges: [] },
      },
    ],
  };
}

const simulation = (p: Project, extra: Partial<RunRequest> = {}): RunRequest => ({
  project: p,
  scenarioId: p.scenarios[0].id,
  mode: 'simulation',
  ...extra,
});

function branchProject(passed: boolean): Project {
  const yes = request('yes', '/yes', { side: 'yes' });
  const no = request('no', '/no', { side: 'no' });
  return project(`if-${passed}`, {
    nodes: [
      request('join', '/join', { joined: true }),
      request('yes-next', '/yes/next', { step: 2 }),
      request('no-next', '/no/next', { step: 2 }),
      yes,
      no,
      check('if', 'if', literal(passed), literal(true)),
      node('trigger', 'start'),
      // Data from an inactive branch: never a request, never a failure.
      request(
        'uses-yes',
        '/uses',
        { ok: true },
        { method: 'POST', inputs: { body: reference('yes') } },
      ),
      check('assert', 'checks-yes', reference('yes', 'body', 'side'), literal('yes')),
      node('variable', 'no-side', { inputs: { value: reference('no', 'body', 'side') } }),
      // Active through ordering, but it reads data from the other branch.
      node('select', 'mixed', {
        inputs: { value: reference('yes', 'body') },
        config: { path: 'side' },
      }),
    ],
    edges: [
      edge('start', 'if'),
      edge('if', 'yes', 'true'),
      edge('if', 'no', 'false'),
      edge('yes', 'yes-next'),
      edge('no', 'no-next'),
      edge('yes-next', 'join'),
      edge('no-next', 'join'),
      edge('no', 'mixed'),
    ],
  });
}

function policyProject(errorPolicy: 'stop' | 'continue'): Project {
  return project(
    `policy-${errorPolicy}`,
    {
      nodes: [
        node('trigger', 'start'),
        request(
          'create',
          '/items',
          { id: 'item-1', price: 100 },
          { method: 'POST', inputs: { body: literal({}) } },
        ),
        check('assert', 'price', reference('create', 'body', 'price'), literal(80)),
        request('after', '/after', { ok: true }),
        request('independent', '/independent', { ok: true }),
        node('select', 'reads-failed', { inputs: { value: reference('price', 'value') } }),
      ],
      edges: [
        edge('start', 'create'),
        edge('create', 'price'),
        edge('price', 'after'),
        edge('start', 'independent'),
      ],
    },
    {
      errorPolicy,
      cleanup: {
        nodes: [
          request(
            'delete',
            '/items/:id',
            { deleted: true },
            {
              method: 'DELETE',
              inputs: { id: reference('create', 'body', 'id') },
            },
          ),
        ],
        edges: [],
      },
    },
  );
}

function conditionProject(): Project {
  const listed = node('condition', 'listed', {
    inputs: {
      actual: reference('read', 'body'),
      expected: literal(null),
      price: reference('read', 'body', 'price'),
      target: literal(99.95),
      kind: literal('book'),
      field: { kind: 'parameter', name: 'field' },
    },
    config: {
      predicate: 'equals',
      tolerance: 0.1,
      match: 'all',
      predicates: [
        { input: 'price', expected: 'target', predicate: 'approximately' },
        { input: 'actual', path: 'meta.{{field}}', expected: 'kind', predicate: 'equals' },
      ],
    },
  });
  const anyOf = node('condition', 'any-of', {
    inputs: { actual: reference('read', 'body'), expected: literal(null), wrong: literal('x') },
    config: {
      predicate: 'equals',
      match: 'any',
      predicates: [
        { input: 'actual', path: 'meta.kind', expected: 'wrong', predicate: 'equals' },
        { input: 'actual', path: 'tags', expected: 'wrong', predicate: 'notEmpty' },
      ],
    },
  });
  const broken = node('condition', 'broken-list', {
    inputs: { actual: reference('read', 'body'), expected: literal(null), limit: literal(1) },
    config: {
      predicate: 'equals',
      predicates: [{ input: 'actual', path: 'missing', expected: 'limit', predicate: 'greater' }],
    },
  });
  return project(
    'conditions',
    {
      nodes: [
        request('read', '/read', { price: 100, meta: { kind: 'book' }, tags: ['a'], empty: null }),
        check('condition', 'gate', reference('read', 'body', 'price'), literal(0), {
          predicate: 'less',
        }),
        node('build', 'blocked', { inputs: { field: reference('gate', 'passed') } }),
        node('select', 'independent', {
          inputs: { value: reference('read', 'body') },
          config: { path: 'meta.kind' },
        }),
        listed,
        anyOf,
        broken,
        node('formula', 'after-list', {
          inputs: { left: reference('listed', 'value', 'price'), right: literal(2) },
          config: { operation: 'multiply' },
        }),
        check('assert', 'exists-null', reference('read', 'body', 'empty'), literal(null), {
          predicate: 'exists',
        }),
        check('assert', 'missing', reference('read', 'body', 'absent'), literal(null), {
          predicate: 'exists',
        }),
        check('assert', 'not-empty', reference('read', 'body', 'tags'), literal(null), {
          predicate: 'notEmpty',
        }),
        check('assert', 'type', reference('read', 'body', 'tags'), literal('array'), {
          predicate: 'type',
        }),
        check('assert', 'contains', reference('read', 'body', 'tags'), literal('a'), {
          predicate: 'contains',
        }),
        check('assert', 'not-a-number', reference('read', 'body', 'meta'), literal(1), {
          predicate: 'greater',
        }),
      ],
      edges: [],
    },
    { parameters: { field: 'kind' } },
  );
}

function componentsProject(): Project {
  const double: ComponentDefinition = {
    id: 'double',
    revision: 1,
    name: 'Double',
    description: '',
    inputs: [{ name: 'amount', type: 'number', required: true }],
    outputs: [{ name: 'result', type: 'number', binding: reference('calc', 'value') }],
    graph: {
      nodes: [
        node('formula', 'calc', {
          inputs: { left: { kind: 'parameter', name: 'amount' }, right: literal(2) },
          config: { operation: 'multiply' },
        }),
      ],
      edges: [],
    },
  };
  const tenfold = structuredClone(double);
  tenfold.revision = 2;
  tenfold.graph.nodes[0].inputs.right = literal(10);
  const createThenFail: ComponentDefinition = {
    id: 'create-then-fail',
    revision: 1,
    name: 'Create then fail',
    description: '',
    inputs: [],
    outputs: [
      { name: 'id', type: 'string', binding: reference('create', 'body', 'id') },
      { name: 'missing', type: 'any', binding: reference('broken', 'value') },
    ],
    graph: {
      nodes: [
        request(
          'create',
          '/items',
          { id: 'new' },
          { method: 'POST', inputs: { body: literal({}) } },
        ),
        node('formula', 'broken', {
          inputs: { left: literal(1), right: literal(0) },
          config: { operation: 'divide' },
        }),
      ],
      edges: [],
    },
  };
  const branching: ComponentDefinition = {
    id: 'branching',
    revision: 1,
    name: 'Branching',
    description: '',
    inputs: [{ name: 'flag', type: 'boolean', defaultValue: false }],
    outputs: [{ name: 'side', type: 'any', binding: reference('inner-no', 'body', 'side') }],
    graph: {
      nodes: [
        node('trigger', 'inner-start'),
        check('if', 'inner-if', { kind: 'parameter', name: 'flag' }, literal(true)),
        request('inner-yes', '/inner/yes', { side: 'yes' }),
        request('inner-no', '/inner/no', { side: 'no' }),
      ],
      edges: [
        edge('inner-start', 'inner-if'),
        edge('inner-if', 'inner-yes', 'true'),
        edge('inner-if', 'inner-no', 'false'),
      ],
    },
  };
  const failing: ComponentDefinition = {
    id: 'failing',
    revision: 1,
    name: 'Failing',
    description: '',
    inputs: [],
    outputs: [],
    graph: {
      nodes: [check('assert', 'inner-check', literal(1), literal(2))],
      edges: [],
    },
  };
  const instance = (
    id: string,
    definition: ComponentDefinition,
    inputs: Record<string, Binding> = {},
  ) =>
    node('component', id, {
      inputs,
      config: { componentId: definition.id, revision: definition.revision },
    });
  return project(
    'components',
    {
      nodes: [
        instance('double-1', double, { amount: literal(3) }),
        instance('double-2', tenfold, { amount: literal(3) }),
        instance('wrong-type', double, { amount: literal('3') }),
        instance('creator', createThenFail),
        node('select', 'next', { inputs: { value: reference('creator', 'id') } }),
        instance('branching', branching),
        instance('failing', failing),
      ],
      edges: [],
    },
    {
      components: [double, tenfold, createThenFail, branching, failing],
      cleanup: {
        nodes: [
          request(
            'cleanup',
            '/items/:id',
            { deleted: true },
            {
              method: 'DELETE',
              inputs: { id: reference('creator', 'id') },
            },
          ),
          request(
            'cleanup-missing',
            '/items/:id',
            { deleted: true },
            {
              method: 'DELETE',
              inputs: { id: reference('creator', 'missing') },
            },
          ),
        ],
        edges: [],
      },
    },
  );
}

function depthProject(): Project {
  const components: ComponentDefinition[] = [];
  for (let level = 17; level >= 1; level--) {
    const inner =
      level === 17
        ? node('variable', `leaf`, { inputs: { value: literal('deep') } })
        : node('component', `level-${level + 1}`, {
            config: { componentId: `level-${level + 1}`, revision: 1 },
          });
    components.push({
      id: `level-${level}`,
      revision: 1,
      name: `Level ${level}`,
      description: '',
      inputs: [],
      outputs: [],
      graph: { nodes: [inner], edges: [] },
    });
  }
  return project(
    'component-depth',
    {
      nodes: [node('component', 'top', { config: { componentId: 'level-1', revision: 1 } })],
      edges: [],
    },
    { components },
  );
}

function cleanupProject(withStart: boolean): Project {
  const cleanupNodes: TestNode[] = [
    request(
      'delete',
      '/items/:id',
      { deleted: true },
      {
        method: 'DELETE',
        inputs: { id: reference('create', 'body', 'id') },
      },
    ),
    request(
      'delete-missing',
      '/items/:id',
      { deleted: true },
      {
        method: 'DELETE',
        inputs: { id: reference('create', 'body', 'absent') },
      },
    ),
    request('get-with-body', '/broken', { never: true }, { inputs: { body: literal({ a: 1 }) } }),
    request('after-error', '/after', { ok: true }),
    request(
      'uses-error',
      '/errors/:value',
      { ok: true },
      { inputs: { value: reference('broken', 'value') } },
    ),
    request(
      'uses-blocked',
      '/blocked',
      { ok: true },
      { inputs: { body: reference('read', 'body') } },
    ),
    request('no-sample', '/no-sample', undefined),
  ];
  const edges = [edge('get-with-body', 'after-error'), edge('delete', 'uses-blocked')];
  if (withStart) {
    cleanupNodes.unshift(node('trigger', 'cleanup-start'));
    edges.push(edge('cleanup-start', 'delete'), edge('cleanup-start', 'get-with-body'));
  }
  return project(
    withStart ? 'cleanup-with-start' : 'cleanup-rules',
    {
      nodes: [
        request(
          'create',
          '/items',
          { id: 'item-7' },
          { method: 'POST', inputs: { body: literal({ n: 1 }) } },
        ),
        check('assert', 'fails', reference('create', 'body', 'id'), literal('other')),
        node('formula', 'broken', {
          inputs: { left: literal(1), right: literal(0) },
          config: { operation: 'divide' },
        }),
        check('condition', 'gate', literal(1), literal(2)),
        request('read', '/read', { ok: true }),
      ],
      edges: [edge('gate', 'read')],
    },
    { cleanup: { nodes: cleanupNodes, edges } },
  );
}

function diagnosticsProject(): Project {
  return project('diagnostics', {
    nodes: [
      node('trigger', 'start'),
      request('first', '/first', { id: 'item', price: 100 }),
      check('assert', 'verify', reference('first', 'body', 'price'), literal(100)),
      request('unrelated', '/unrelated', { ok: true }),
      request('disconnected', '/disconnected/{{region}}', { ok: true }),
      node('variable', 'price-variable', {
        inputs: { value: reference('first', 'body', 'price') },
      }),
      check('assert', 'via-variable', reference('price-variable', 'value'), literal(100)),
    ],
    edges: [
      edge('start', 'first'),
      edge('first', 'verify'),
      edge('start', 'unrelated'),
      edge('first', 'via-variable'),
    ],
  });
}

function redactionProject(): Project {
  const post = request(
    'post',
    '/items?key={{API_KEY}}',
    { id: 'new', token: 'response-token-value' },
    {
      method: 'POST',
      inputs: {
        key: { kind: 'environment', name: 'API_KEY' },
        body: literal({ secret: 'body {{key}}', plain: 'uses {{key}}', note: 'visible' }),
      },
    },
  );
  post.config.headers = { Authorization: 'Bearer {{API_KEY}}', 'X-Trace': 'trace-{{key}}' };
  post.config.query = { api_key: '{{key}}', page: '2' };
  const echo = request('echo', '/echo', {
    echoed: 'my+session secret',
    encoded: 'my%2Bsession%20secret',
  });
  const credentials = request('credentials', 'https://user:pa%20ss@api.example.test/', {
    never: true,
  });
  const variable = node('variable', 'session', {
    inputs: { value: { kind: 'environment', name: 'API_KEY' } },
  });
  const compare = check(
    'assert',
    'compare',
    reference('session', 'value'),
    literal('my+session secret'),
  );
  const hardcoded = request('hardcoded', '/?api-key=hardcoded+query+secret&x=1', { ok: true });
  const reads = check(
    'assert',
    'reads-token',
    reference('post', 'body', 'token'),
    literal('response-token-value'),
  );
  return project(
    'redaction',
    { nodes: [post, echo, credentials, variable, compare, hardcoded, reads], edges: [] },
    {
      environment: {
        baseUrl: 'https://api.example.test',
        variables: { API_KEY: 'stored-secret-value', PUBLIC: 'public' },
        secretNames: ['API_KEY'],
      },
    },
  );
}

function valuesProject(): Project {
  return project(
    'values',
    {
      nodes: [
        request('source', '/source', {
          items: [{ price: 0, flag: false, optional: null }],
          name: 'Zażółć',
          n: 7,
        }),
        node('variable', 'variable', {
          inputs: { value: reference('source', 'body', 'items[0]') },
        }),
        node('select', 'templated-select', {
          inputs: { value: reference('source', 'body'), index: literal(0) },
          config: { path: 'items[{{index}}].price' },
        }),
        node('select', 'missing-select', {
          inputs: { value: reference('source', 'body') },
          config: { path: 'absent.deeper' },
        }),
        node('build', 'array', {
          inputs: { b: literal('b'), '10': literal(10), a: literal('a'), '2': literal(2) },
          config: { buildType: 'array' },
        }),
        node('build', 'object', {
          inputs: { z: literal(1), '1': reference('source', 'body', 'n'), y: literal(null) },
          config: { buildType: 'object' },
        }),
        node('build', 'incomplete', {
          inputs: { a: reference('source', 'body', 'absent') },
          config: { buildType: 'object' },
        }),
        node('formula', 'random', {
          inputs: { left: literal(1), right: literal(6) },
          config: { operation: 'random' },
        }),
        node('formula', 'random-real', {
          inputs: { left: literal(0.5), right: literal(1.5) },
          config: { operation: 'random' },
        }),
        node('formula', 'rounded', {
          inputs: { left: literal(2.5) },
          config: { operation: 'round' },
        }),
        node('formula', 'missing-left', {
          inputs: { left: reference('source', 'body', 'absent'), right: literal(1) },
          config: { operation: 'add' },
        }),
        node('formula', 'wrong-type', {
          inputs: { left: literal('1'), right: literal(1) },
          config: { operation: 'add' },
        }),
        node('formula', 'null-operand', {
          inputs: { left: reference('source', 'body', 'items[0].optional'), right: literal(2) },
          config: { operation: 'multiply' },
        }),
        node('formula', 'nested', {
          inputs: {
            left: {
              kind: 'expression',
              operator: 'add',
              inputs: {
                left: {
                  kind: 'expression',
                  operator: 'length',
                  inputs: { left: reference('source', 'body', 'name') },
                },
                right: { kind: 'parameter', name: 'bonus' },
              },
            },
            right: literal(3),
          },
          config: { operation: 'multiply' },
        }),
        node('formula', 'nested-error', {
          inputs: {
            left: {
              kind: 'expression',
              operator: 'add',
              inputs: {
                left: {
                  kind: 'expression',
                  operator: 'divide',
                  inputs: { left: literal(1), right: literal(0) },
                },
                right: literal(1),
              },
            },
            right: literal(3),
          },
          config: { operation: 'multiply' },
        }),
        request('get-with-body', '/x', { ok: true }, { inputs: { body: literal({}) } }),
        request('no-sample', '/x', undefined),
        request('missing-template', '/items/{{nothing}}', { ok: true }),
        request('missing-path-argument', '/items/:id', { ok: true }),
        request(
          'typed-body',
          '/items/:id/{{name}}',
          { ok: true },
          {
            method: 'PUT',
            inputs: {
              id: literal('a b/ż'),
              name: reference('source', 'body', 'name'),
              amount: literal(20),
              body: literal({
                amount: '{{amount}}',
                label: '#{{amount}}',
                list: ['{{id}}'],
                nested: { r: '{{region}}' },
              }),
            },
            sampleStatus: 422,
          },
        ),
        request(
          'absolute',
          'HTTPS://other.example.test/path?q=1',
          { ok: true },
          { method: 'HEAD' },
        ),
        request('ftp', 'ftp://files.example.test/x', { ok: true }),
        request('invalid-url', 'http://[', { ok: true }),
        request('required-port', '/ports', { ok: true }),
        check('assert', 'approximately', literal(1.05), literal(1), {
          predicate: 'approximately',
          tolerance: 0.1,
        }),
        check(
          'assert',
          'not-equals',
          reference('source', 'body', 'items'),
          literal([{ price: 0, flag: false, optional: null }]),
          { predicate: 'notEquals' },
        ),
      ],
      edges: [],
    },
    { parameters: { bonus: 1 } },
  );
}

function withRequiredPort(p: Project): Project {
  const required = p.scenarios[0].graph.nodes.find((n) => n.id === 'required-port')!;
  required.config.inputPorts = [
    { name: 'needed', type: 'string', required: true },
    { name: 'typed', type: 'number', defaultValue: 5 },
  ];
  required.config.query = { needed: '{{needed}}', typed: '{{typed}}' };
  required.inputs = { needed: reference('source', 'body', 'name') };
  required.config.headers = { 'X-Typed': '{{typed}}' };
  return p;
}

function mergeProject(): Project {
  return project('merge-error', {
    nodes: [
      node('trigger', 'start'),
      request('broken', 'http://[', { never: true }),
      request('good', '/good', { ok: true }),
      request('join', '/join', { ok: true }),
      check('if', 'decide', literal(1), literal(2)),
      request('skipped-alternative', '/skipped', { ok: true }),
      request('after-alternatives', '/after', { ok: true }),
    ],
    edges: [
      edge('start', 'broken'),
      edge('start', 'good'),
      edge('broken', 'join'),
      edge('good', 'join'),
      edge('start', 'decide'),
      edge('decide', 'skipped-alternative', 'true'),
      edge('skipped-alternative', 'after-alternatives'),
      edge('good', 'after-alternatives'),
    ],
  });
}

function legacyProject(): Project {
  return project(
    'legacy',
    {
      nodes: [
        check('assert', 'verify', reference('second', 'body', 'n'), literal(2)),
        request(
          'second',
          '/second/{{id}}',
          { n: 2 },
          { inputs: { id: reference('first', 'body', 'id') } },
        ),
        request('first', '/first', { id: 'x/1' }),
        request('independent', '/independent', { ok: true }),
        node('variable', 'parameter', { inputs: { value: { kind: 'parameter', name: 'limit' } } }),
      ],
      edges: [edge('first', 'second')],
    },
    { parameters: { limit: 0 } },
  );
}

function demoCases(normalize: Normalize): RunCase[] {
  const sources = [
    ['shop', createShopProject],
    ['tickets', createTicketsProject],
    ['blocks', createBlocksProject],
    ['workflow', createWorkflowProject],
  ] as const;
  return sources.flatMap(([slug, create]) => {
    const normalized = normalize(create(), slug);
    return normalized.scenarios.map((scenario) => ({
      slug: `demo-${slug}-${scenario.id}`,
      description: `Demo project ${slug}, scenario ${scenario.id}.`,
      request: { project: normalized, scenarioId: scenario.id, mode: 'simulation' as const },
    }));
  });
}

function stripVolatile(run: RunResult): RunResult {
  const steps = (list: StepResult[]): StepResult[] =>
    list.map((step) => ({
      ...step,
      durationMs: 0,
      ...(step.children ? { children: steps(step.children) } : {}),
    }));
  return {
    ...run,
    id: 'run-00000001',
    startedAt: HISTORY_TIME,
    finishedAt: HISTORY_TIME,
    steps: steps(run.steps),
  };
}

function normalizeStep(step: StepResult): Record<string, unknown> {
  return {
    nodeId: step.nodeId,
    name: step.name,
    kind: step.kind,
    status: step.status,
    phase: step.phase,
    inputs: step.inputs,
    outputs: step.outputs,
    ...('actual' in step ? { actual: step.actual } : {}),
    ...('expected' in step ? { expected: step.expected } : {}),
    ...(step.request ? { request: step.request } : {}),
    ...(step.children ? { children: step.children.map(normalizeStep) } : {}),
  };
}

function normalizeRun(run: RunResult, single: boolean): Record<string, unknown> {
  return {
    status: run.status,
    diagnostic: run.diagnostic,
    // A single step stores its provenance in `error`; the port uses `notice`.
    ...(single ? {} : { hasError: run.error !== undefined }),
    steps: run.steps.map(normalizeStep),
  };
}

async function execute(request: RunRequest): Promise<RunResult> {
  const original = Math.random;
  Math.random = createRng(RANDOM_SEED);
  try {
    return await runScenario(request);
  } finally {
    Math.random = original;
  }
}

export async function buildRunFixtures(
  normalize: Normalize,
  toJsonText: (value: unknown) => string,
): Promise<Map<string, string>> {
  const cases: RunCase[] = demoCases(normalize);
  const add = (slug: string, description: string, build: (p: Project) => RunRequest, p: Project) =>
    cases.push({ slug, description, request: build(normalize(p, slug)) });

  add(
    'if-true',
    'IF true runs only the yes branch and merges alternatives.',
    (p) => simulation(p),
    branchProject(true),
  );
  add(
    'if-false',
    'IF false runs only the no branch; inactive data never runs a request.',
    (p) => simulation(p),
    branchProject(false),
  );
  add(
    'legacy-without-start',
    'A graph without Start runs every root in dependency order.',
    (p) => simulation(p),
    legacyProject(),
  );
  add(
    'error-policy-stop',
    'Stop skips the rest of the main phase but cleans up.',
    (p) => simulation(p),
    policyProject('stop'),
  );
  add(
    'error-policy-continue',
    'Continue keeps a failed assertion usable.',
    (p) => simulation(p),
    policyProject('continue'),
  );
  add(
    'conditions',
    'Conditions block descendants; predicate lists with all/any and tolerance.',
    (p) => simulation(p),
    conditionProject(),
  );
  add(
    'components',
    'Pinned revisions, nested IF, failures and exported IDs used by cleanup.',
    (p) => simulation(p),
    componentsProject(),
  );
  add(
    'component-depth',
    'Component nesting deeper than 16 levels is an error.',
    (p) => simulation(p),
    depthProject(),
  );
  add(
    'cleanup-rules',
    'Cleanup ordering means completion; missing inputs skip steps.',
    (p) => simulation(p),
    cleanupProject(false),
  );
  add(
    'cleanup-with-start',
    'Start reachability inside the cleanup graph.',
    (p) => simulation(p),
    cleanupProject(true),
  );
  add(
    'merge-error',
    'An error on an active alternative is not hidden by another one.',
    (p) => simulation(p),
    mergeProject(),
  );
  add(
    'values',
    'Values, templates, operators, request preparation and errors.',
    (p) => simulation(p),
    withRequiredPort(valuesProject()),
  );
  add(
    'redaction',
    'Secrets are masked in every published field.',
    (p) => simulation(p, { secrets: { API_KEY: 'my+session secret' } }),
    redactionProject(),
  );
  add(
    'target',
    'Running up to a node selects its dependency closure.',
    (p) => simulation(p, { targetNodeId: 'via-variable' }),
    diagnosticsProject(),
  );
  add(
    'single-without-history',
    'A single step outside the Start path bypasses its own gate.',
    (p) => simulation(p, { singleNodeId: 'disconnected' }),
    diagnosticsProject(),
  );

  // A single step with history replays a normalised earlier diagnostic run.
  const diagnostics = normalize(diagnosticsProject(), 'single-with-history');
  const previous = stripVolatile(
    await execute(simulation(diagnostics, { targetNodeId: 'verify' })),
  );
  cases.push({
    slug: 'single-with-history',
    description: 'A single step reads its inputs from an earlier run.',
    request: simulation(diagnostics, { singleNodeId: 'verify', previousRun: previous }),
  });
  const workflow = normalize(createWorkflowProject(), 'workflow');
  const previousWorkflow = stripVolatile(
    await execute(simulation(workflow, { targetNodeId: 'flow-check' })),
  );
  cases.push({
    slug: 'single-workflow-add',
    description: 'A single request step with historical inputs in the cart workflow.',
    request: simulation(workflow, { singleNodeId: 'flow-add', previousRun: previousWorkflow }),
  });

  const files = new Map<string, string>();
  for (const runCase of cases) {
    const errors = validateProject(runCase.request.project);
    if (errors.length)
      throw new Error(`Invalid run fixture "${runCase.slug}": ${errors.join('; ')}`);
    const run = await execute(runCase.request);
    files.set(
      `runs/${runCase.slug}.json`,
      toJsonText({
        description: runCase.description,
        randomSeed: RANDOM_SEED,
        request: runCase.request,
        expected: normalizeRun(run, !!runCase.request.singleNodeId),
      }),
    );
  }
  return files;
}

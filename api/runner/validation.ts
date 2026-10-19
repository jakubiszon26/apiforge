import { isOperator } from '../../shared/operators';
import type { RunRequest } from '../../shared/model';
import { validateProject } from '../../shared/graph';

const kinds = new Set([
  'trigger',
  'if',
  'variable',
  'request',
  'select',
  'build',
  'formula',
  'assert',
  'condition',
  'component',
]);
const predicates = new Set([
  'equals',
  'notEquals',
  'greater',
  'less',
  'exists',
  'notEmpty',
  'contains',
  'type',
  'approximately',
]);
const types = new Set(['any', 'string', 'number', 'boolean', 'object', 'array']);
const record = (v: unknown): v is Record<string, any> =>
  !!v && typeof v === 'object' && !Array.isArray(v);
const text = (v: unknown): v is string => typeof v === 'string' && v.length <= 100_000;
function requireValue(ok: unknown, message: string): asserts ok {
  if (!ok) throw new Error(message);
}
function json(v: unknown, depth = 0): void {
  requireValue(depth < 40, 'Zbyt głęboko zagnieżdżone dane.');
  if (
    v === null ||
    typeof v === 'boolean' ||
    text(v) ||
    (typeof v === 'number' && Number.isFinite(v))
  )
    return;
  requireValue(Array.isArray(v) || record(v), 'Nieprawidłowa wartość JSON.');
  for (const [key, value] of Object.entries(v)) {
    requireValue(
      !['__proto__', 'constructor', 'prototype'].includes(key),
      'Niedozwolona nazwa pola.',
    );
    json(value, depth + 1);
  }
}
function binding(b: any, depth = 0) {
  requireValue(depth < 12, 'Zbyt głęboko zagnieżdżone bloki.');
  requireValue(record(b), 'Nieprawidłowe powiązanie.');
  if (b.kind === 'expression') {
    requireValue(isOperator(b.operator) && record(b.inputs), 'Nieprawidłowy operator.');
    Object.values(b.inputs).forEach((child) => binding(child, depth + 1));
  } else if (b.kind === 'literal') {
    requireValue(Object.hasOwn(b, 'value'), 'Brak literału.');
    json(b.value);
  } else if (b.kind === 'reference')
    requireValue(text(b.nodeId) && text(b.port) && text(b.path), 'Nieprawidłowe źródło danych.');
  else
    requireValue(
      ['parameter', 'environment'].includes(b.kind) && text(b.name),
      'Nieprawidłowe powiązanie.',
    );
}
function ports(list: any) {
  requireValue(Array.isArray(list) && list.length <= 100, 'Nieprawidłowe porty.');
  const names = new Set();
  for (const p of list) {
    requireValue(
      record(p) && text(p.name) && p.name.length && types.has(p.type) && !names.has(p.name),
      'Nieprawidłowy lub powtórzony port.',
    );
    names.add(p.name);
  }
}
function graph(g: any) {
  requireValue(
    record(g) &&
      Array.isArray(g.nodes) &&
      g.nodes.length <= 200 &&
      Array.isArray(g.edges) &&
      g.edges.length <= 1000,
    'Graf przekracza limit lub jest nieprawidłowy.',
  );
  for (const n of g.nodes) {
    requireValue(
      record(n) &&
        text(n.id) &&
        text(n.name) &&
        kinds.has(n.kind) &&
        record(n.inputs) &&
        record(n.config) &&
        record(n.position),
      'Nieprawidłowy klocek.',
    );
    requireValue(
      Number.isFinite(n.position.x) && Number.isFinite(n.position.y),
      'Nieprawidłowa pozycja klocka.',
    );
    requireValue(Object.keys(n.inputs).length <= 100, 'Zbyt wiele wejść.');
    Object.values(n.inputs).forEach((value) => binding(value));
    const c = n.config;
    if (c.templateInputs !== undefined) ports(c.templateInputs);
    if (n.kind === 'request') {
      requireValue(
        text(c.url) &&
          c.url.length > 0 &&
          ['GET', 'HEAD', 'POST', 'PUT', 'PATCH', 'DELETE', 'OPTIONS'].includes(c.method ?? 'GET'),
        'Nieprawidłowy request HTTP.',
      );
      for (const key of ['headers', 'query'])
        if (c[key] !== undefined)
          requireValue(
            record(c[key]) && Object.values(c[key]).every(text),
            'Nagłówki i query muszą zawierać tekst.',
          );
      if (c.timeoutMs !== undefined)
        requireValue(
          Number.isFinite(c.timeoutMs) && c.timeoutMs > 0 && c.timeoutMs <= 60000,
          'Timeout musi mieścić się w 1–60000 ms.',
        );
      if (c.inputPorts !== undefined) ports(c.inputPorts);
    }
    if (n.kind === 'formula') requireValue(isOperator(c.operation), 'Nieznana formuła.');
    if (n.kind === 'assert' || n.kind === 'condition' || n.kind === 'if') {
      requireValue(predicates.has(c.predicate ?? 'equals'), 'Nieznany predykat.');
      if (c.tolerance !== undefined)
        requireValue(
          typeof c.tolerance === 'number' && c.tolerance >= 0,
          'Nieprawidłowa tolerancja.',
        );
      if (c.predicates !== undefined)
        requireValue(
          Array.isArray(c.predicates) &&
            c.predicates.length <= 100 &&
            c.predicates.every(
              (p: any) =>
                record(p) &&
                text(p.input) &&
                text(p.expected) &&
                predicates.has(p.predicate) &&
                (p.path === undefined || text(p.path)),
            ),
          'Nieprawidłowe warunki.',
        );
      if (c.match !== undefined)
        requireValue(['all', 'any'].includes(c.match), 'Nieprawidłowe łączenie warunków.');
    }
    if (c.path !== undefined) requireValue(text(c.path), 'Nieprawidłowa ścieżka JSON.');
    if (n.kind === 'component')
      requireValue(
        text(c.componentId) && Number.isInteger(c.revision) && c.revision >= 1,
        'Nieprawidłowa rewizja komponentu.',
      );
  }
  for (const e of g.edges)
    requireValue(
      record(e) && text(e.id) && text(e.source) && text(e.target),
      'Nieprawidłowe połączenie.',
    );
}

export function validateRunRequest(value: unknown): asserts value is RunRequest {
  json(value);
  requireValue(record(value) && record(value.project), 'Brak projektu.');
  const p = value.project;
  requireValue(
    Array.isArray(p.scenarios) &&
      p.scenarios.length <= 100 &&
      Array.isArray(p.components) &&
      p.components.length <= 200 &&
      Array.isArray(p.contracts),
    'Nieprawidłowy projekt.',
  );
  for (const s of p.scenarios) {
    requireValue(
      record(s) && text(s.id) && text(s.name) && record(s.parameters),
      'Nieprawidłowy scenariusz.',
    );
    graph(s.graph);
    graph(s.cleanup);
  }
  const componentIds = new Set();
  for (const c of p.components) {
    requireValue(
      record(c) && text(c.id) && text(c.name) && Number.isInteger(c.revision) && c.revision >= 1,
      'Nieprawidłowy komponent.',
    );
    requireValue(!componentIds.has(`${c.id}@${c.revision}`), 'Powtórzona rewizja komponentu.');
    componentIds.add(`${c.id}@${c.revision}`);
    ports(c.inputs);
    ports(c.outputs);
    c.outputs.forEach((o: any) => binding(o.binding));
    graph(c.graph);
  }
  requireValue(
    record(p.environment) &&
      text(p.environment.baseUrl) &&
      record(p.environment.variables) &&
      Object.values(p.environment.variables).every(text) &&
      Array.isArray(p.environment.secretNames) &&
      p.environment.secretNames.every(text),
    'Nieprawidłowe środowisko.',
  );
  requireValue(
    ['live', 'simulation'].includes(value.mode) && text(value.scenarioId),
    'Nieprawidłowy tryb uruchomienia.',
  );
  requireValue(!value.targetNodeId || text(value.targetNodeId), 'Nieprawidłowy cel.');
  requireValue(!value.singleNodeId || text(value.singleNodeId), 'Nieprawidłowy krok.');
  requireValue(
    !(value.targetNodeId && value.singleNodeId),
    'Wybierz jeden tryb częściowego wykonania.',
  );
  if (value.secrets !== undefined)
    requireValue(
      record(value.secrets) && Object.values(value.secrets).every(text),
      'Nieprawidłowe sekrety.',
    );
  const errors = validateProject(p);
  requireValue(!errors.length, errors.join(' '));
  const s = p.scenarios.find((s: any) => s.id === value.scenarioId);
  requireValue(s, 'Nie znaleziono scenariusza.');
  for (const id of [value.targetNodeId, value.singleNodeId])
    if (id)
      requireValue(
        s.graph.nodes.some((n: any) => n.id === id),
        'Nie znaleziono wybranego klocka.',
      );
  if (value.previousRun !== undefined) {
    const prev = value.previousRun;
    requireValue(
      value.singleNodeId,
      'Historyczne wejścia są dostępne tylko dla pojedynczego kroku.',
    );
    requireValue(
      record(prev) &&
        text(prev.id) &&
        prev.id.length > 0 &&
        prev.scenarioId === value.scenarioId &&
        prev.mode === value.mode &&
        ['passed', 'failed', 'cancelled'].includes(prev.status) &&
        text(prev.finishedAt) &&
        Number.isFinite(Date.parse(prev.finishedAt)) &&
        Array.isArray(prev.steps) &&
        prev.steps.length <= 400,
      'Nieprawidłowy wcześniejszy przebieg.',
    );
    const ids = new Set<string>();
    for (const step of prev.steps) {
      requireValue(
        record(step) &&
          text(step.nodeId) &&
          record(step.outputs) &&
          ['main', 'cleanup'].includes(step.phase) &&
          ['passed', 'failed', 'error', 'blocked', 'skipped', 'cancelled'].includes(step.status),
        'Nieprawidłowe zapisane wyjścia.',
      );
      requireValue(!ids.has(step.nodeId), 'Powtórzone ID historycznego kroku.');
      ids.add(step.nodeId);
    }
  }
}

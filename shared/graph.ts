import { operators, isOperator } from './operators';
import { bindingLeaves, bindingEdgeId, references } from './bindings';
import { literal, uid } from './model';
import type {
  Binding,
  ComponentDefinition,
  Graph,
  NodeKind,
  Port,
  Project,
  TestNode,
} from './model';

export const kindLabels: Record<NodeKind, string> = {
  trigger: 'Start',
  if: 'IF / ELSE',
  variable: 'Zmienna',
  request: 'Request HTTP',
  select: 'Wybierz dane',
  build: 'Zbuduj dane',
  formula: 'Operator',
  assert: 'Porównanie',
  condition: 'Warunek danych',
  component: 'Komponent',
};
export const kindDescriptions: Record<NodeKind, string> = {
  trigger: 'Uruchom po kliknięciu Uruchom',
  if: 'Wybierz gałąź: tak albo nie',
  variable: 'Nazwana wartość do użycia w polach',
  request: 'Dowolny endpoint REST API',
  select: 'Wybierz pole z odpowiedzi',
  build: 'Przygotuj obiekt lub listę',
  formula: 'Matematyka, logika, tekst i dane',
  assert: 'Sprawdź otrzymany wynik',
  condition: 'Sprawdź warunki scenariusza',
  component: 'Własny zestaw kroków',
};
export function definition(project: Project, node: TestNode): ComponentDefinition | undefined {
  return project.components.find(
    (c) => c.id === node.config.componentId && c.revision === node.config.revision,
  );
}
export function inputPorts(node: TestNode, project: Project): Port[] {
  const standard = builtinInputPorts(node, project);
  return [
    ...standard,
    ...(node.config.templateInputs ?? []).filter(
      (port) => !standard.some((input) => input.name === port.name),
    ),
  ];
}
function builtinInputPorts(node: TestNode, project: Project): Port[] {
  if (node.kind === 'trigger') return [];
  if (node.kind === 'variable')
    return [
      { name: 'value', label: 'Wartość zmiennej', type: 'any', required: true, exposed: true },
    ];
  if (node.kind === 'component') return definition(project, node)?.inputs ?? [];
  if (node.kind === 'request')
    return [
      ...(node.config.inputPorts ?? []),
      ...(!node.config.inputPorts?.some((p) => p.name === 'body')
        ? [{ name: 'body', type: 'any' as const, required: false }]
        : []),
    ];
  if (node.kind === 'select')
    return [{ name: 'value', label: 'Dane', type: 'any', required: true }];
  if (node.kind === 'build')
    return Object.keys(node.inputs).map((name) => ({ name, type: 'any', required: true }));
  if (node.kind === 'formula') return operators[node.config.operation ?? 'multiply'].inputs;
  const standard: Port[] = [
    { name: 'actual', label: 'Wartość otrzymana', type: 'any', required: true },
    {
      name: 'expected',
      label: 'Wartość oczekiwana',
      type: 'any',
      required: !['exists', 'notEmpty'].includes(node.config.predicate ?? 'equals'),
    },
  ];
  return [
    ...standard,
    ...Object.keys(node.inputs)
      .filter((k) => !standard.some((p) => p.name === k))
      .map((name) => ({ name, type: 'any' as const })),
  ];
}
export function outputPorts(node: TestNode, project: Project): Port[] {
  if (node.kind === 'trigger') return [];
  if (node.kind === 'component') return definition(project, node)?.outputs ?? [];
  if (node.kind === 'request')
    return [
      { name: 'body', label: 'Odpowiedź', type: 'any', contractId: node.config.contractId },
      { name: 'status', label: 'Status HTTP', type: 'number' },
      { name: 'headers', label: 'Nagłówki', type: 'object' },
      { name: 'duration', label: 'Czas', type: 'number' },
    ];
  if (node.kind === 'assert' || node.kind === 'condition' || node.kind === 'if')
    return [
      { name: 'value', type: 'any' },
      { name: 'passed', type: 'boolean' },
    ];
  return [
    {
      name: 'value',
      type:
        node.kind === 'formula'
          ? operators[node.config.operation ?? 'multiply'].output
          : node.kind === 'build'
            ? node.config.buildType === 'array'
              ? 'array'
              : 'object'
            : 'any',
    },
  ];
}
export function createNode(
  kind: NodeKind,
  position = { x: 100, y: 100 },
  component?: ComponentDefinition,
): TestNode {
  const node: TestNode = {
    id: uid(),
    kind,
    name: kindLabels[kind],
    position,
    inputs: {},
    config: {},
  };
  // Request arguments belong to the author; endpoint paths need no identifier.
  if (kind === 'request')
    node.config = {
      method: 'GET',
      url: '/endpoint',
      headers: {},
      query: {},
      timeoutMs: 10000,
      inputPorts: [],
    };
  if (kind === 'variable') node.inputs.value = literal('');
  if (kind === 'select') {
    node.inputs.value = literal({});
    node.config.path = '';
  }
  if (kind === 'build') {
    node.config.buildType = 'object';
    node.inputs.field = literal('');
  }
  if (kind === 'formula') {
    node.config.operation = 'multiply';
    node.inputs = { left: literal(0), right: literal(1) };
  }
  if (kind === 'assert' || kind === 'condition' || kind === 'if') {
    node.config.predicate = 'equals';
    node.inputs = { actual: literal(null), expected: literal(null) };
  }
  if (component) {
    node.name = component.name;
    node.config = { componentId: component.id, revision: component.revision };
    for (const p of component.inputs)
      if (p.defaultValue !== undefined) node.inputs[p.name] = literal(p.defaultValue);
  }
  return node;
}
export interface VisualEdge {
  id: string;
  source: string;
  target: string;
  branch?: 'true' | 'false';
  sourcePort?: string;
  targetInput?: string;
  kind: 'data' | 'order';
}
export function visualEdges(graph: Graph): VisualEdge[] {
  return [
    ...graph.edges.map((e) => ({ ...e, kind: 'order' as const })),
    ...graph.nodes.flatMap((n) =>
      Object.entries(n.inputs).flatMap(([name, binding]) =>
        bindingLeaves(binding).flatMap(({ binding: ref, path }) =>
          ref.kind === 'reference'
            ? [
                {
                  id: bindingEdgeId(n.id, name, path),
                  source: ref.nodeId,
                  target: n.id,
                  sourcePort: ref.port,
                  targetInput: name,
                  kind: 'data' as const,
                },
              ]
            : [],
        ),
      ),
    ),
  ];
}
export function dependencies(graph: Graph, nodeId: string): string[] {
  return [
    ...new Set(
      visualEdges(graph)
        .filter((e) => e.target === nodeId)
        .map((e) => e.source),
    ),
  ];
}
export function hasCycle(graph: Graph): boolean {
  const visiting = new Set<string>(),
    done = new Set<string>();
  function visit(id: string): boolean {
    if (visiting.has(id)) return true;
    if (done.has(id)) return false;
    visiting.add(id);
    for (const p of dependencies(graph, id)) if (visit(p)) return true;
    visiting.delete(id);
    done.add(id);
    return false;
  }
  return graph.nodes.some((n) => visit(n.id));
}
export function compatible(source: Port, target: Port): boolean {
  if (target.contractId) return source.contractId === target.contractId;
  return source.type === 'any' || target.type === 'any' || source.type === target.type;
}
export function bindInput(graph: Graph, targetId: string, input: string, binding: Binding): Graph {
  return {
    ...graph,
    nodes: graph.nodes.map((n) =>
      n.id === targetId ? { ...n, inputs: { ...n.inputs, [input]: binding } } : n,
    ),
  };
}
export function deleteNodes(graph: Graph, ids: string[]): Graph {
  return {
    nodes: graph.nodes
      .filter((n) => !ids.includes(n.id))
      .map((n) => ({
        ...n,
        inputs: Object.fromEntries(
          Object.entries(n.inputs).filter(
            ([, b]) => !references(b).some((ref) => ids.includes(ref.nodeId)),
          ),
        ),
      })),
    edges: graph.edges.filter((e) => !ids.includes(e.source) && !ids.includes(e.target)),
  };
}
export function appendPath(path: string, key: string | number): string {
  return typeof key === 'number'
    ? `${path}[${key}]`
    : /^[A-Za-z_$][\w$-]*$/.test(key)
      ? `${path ? `${path}.` : ''}${key}`
      : `${path}[${JSON.stringify(key)}]`;
}
export function getPath(value: unknown, path: string): unknown {
  if (!path) return value;
  const parts: string[] = [];
  let index = 0,
    expectSegment = true;
  while (index < path.length) {
    if (path[index] === '.') {
      if (expectSegment) return undefined;
      expectSegment = true;
      index++;
      continue;
    }
    if (path[index] === '[') {
      const match = path.slice(index).match(/^\[(\d+|"(?:[^"\\]|\\.)*")\]/);
      if (!match) return undefined;
      try {
        parts.push(match[1].startsWith('"') ? JSON.parse(match[1]) : String(Number(match[1])));
      } catch {
        return undefined;
      }
      index += match[0].length;
      expectSegment = false;
      continue;
    }
    if (!expectSegment) return undefined;
    const match = path.slice(index).match(/^[^.[\]]+/);
    if (!match) return undefined;
    parts.push(match[0]);
    index += match[0].length;
    expectSegment = false;
  }
  if (expectSegment) return undefined;
  let result = value;
  for (const part of parts) {
    if (['__proto__', 'constructor', 'prototype'].includes(part)) return undefined;
    if (result === null || typeof result !== 'object' || !Object.hasOwn(result, part))
      return undefined;
    result = (result as Record<string, unknown>)[part];
  }
  return result;
}
export function validateProject(value: unknown): string[] {
  const errors: string[] = [];
  const object = (v: unknown): v is Record<string, unknown> =>
    !!v && typeof v === 'object' && !Array.isArray(v);
  const text = (v: unknown): v is string => typeof v === 'string' && v.length <= 100_000;
  const id = (v: unknown): v is string => text(v) && v.length > 0 && v.length <= 200;
  const json = (v: unknown, depth = 0): boolean =>
    depth < 40 &&
    (v === null ||
      typeof v === 'boolean' ||
      text(v) ||
      (typeof v === 'number' && Number.isFinite(v)) ||
      (Array.isArray(v)
        ? v.length <= 10000 && v.every((x) => json(x, depth + 1))
        : object(v) &&
          Object.entries(v).length <= 10000 &&
          Object.entries(v).every(
            ([k, x]) =>
              !['__proto__', 'constructor', 'prototype'].includes(k) && json(x, depth + 1),
          )));
  const binding = (b: unknown, depth = 0, budget = { count: 0 }): boolean =>
    depth < 12 &&
    ++budget.count <= 200 &&
    object(b) &&
    (b.kind === 'expression'
      ? isOperator(b.operator) &&
        object(b.inputs) &&
        Object.keys(b.inputs).length <= 3 &&
        Object.entries(b.inputs).every(
          ([name, child]) =>
            operators[b.operator as keyof typeof operators].inputs.some(
              (port) => port.name === name,
            ) && binding(child, depth + 1, budget),
        )
      : b.kind === 'literal'
        ? Object.hasOwn(b, 'value') && json(b.value)
        : b.kind === 'reference'
          ? id(b.nodeId) && id(b.port) && text(b.path)
          : ['parameter', 'environment'].includes(String(b.kind)) && id(b.name));
  const ports = (list: unknown, outputs = false): boolean =>
    Array.isArray(list) &&
    list.length <= 100 &&
    list.every(
      (p) =>
        object(p) &&
        id(p.name) &&
        ['any', 'string', 'number', 'boolean', 'object', 'array'].includes(String(p.type)) &&
        (p.defaultValue === undefined || json(p.defaultValue)) &&
        (p.contractId === undefined || id(p.contractId)) &&
        (!outputs || binding(p.binding)),
    ) &&
    new Set(list.map((p) => p.name)).size === list.length;
  if (!object(value)) return ['Projekt musi być obiektem JSON.'];
  if (value.version !== 1) return ['Nieobsługiwana wersja projektu.'];
  if (
    !id(value.id) ||
    !text(value.name) ||
    !Array.isArray(value.scenarios) ||
    !value.scenarios.length ||
    value.scenarios.length > 100 ||
    !Array.isArray(value.components) ||
    value.components.length > 200 ||
    !Array.isArray(value.contracts) ||
    value.contracts.length > 200 ||
    !object(value.environment)
  )
    return ['Niepełna struktura projektu lub przekroczony limit.'];
  const env = value.environment;
  if (
    !text(env.baseUrl) ||
    !object(env.variables) ||
    !Object.values(env.variables).every(text) ||
    !Array.isArray(env.secretNames) ||
    !env.secretNames.every(id)
  )
    return ['Nieprawidłowe środowisko.'];
  if (!value.contracts.every((c) => object(c) && id(c.id) && text(c.name) && json(c.sample)))
    return ['Nieprawidłowy kontrakt danych.'];
  if (new Set(value.contracts.map((c) => (c as { id: string }).id)).size !== value.contracts.length)
    return ['Powtórzone ID kontraktu.'];
  const structuralGraph = (g: unknown, label: string): boolean => {
    if (
      !object(g) ||
      !Array.isArray(g.nodes) ||
      g.nodes.length > 200 ||
      !Array.isArray(g.edges) ||
      g.edges.length > 1000
    ) {
      errors.push(`${label}: nieprawidłowy graf lub przekroczony limit.`);
      return false;
    }
    for (const n of g.nodes) {
      if (
        !object(n) ||
        !id(n.id) ||
        !text(n.name) ||
        !Object.hasOwn(kindLabels, String(n.kind)) ||
        !object(n.inputs) ||
        !object(n.config) ||
        !object(n.position) ||
        !Number.isFinite(n.position.x) ||
        !Number.isFinite(n.position.y)
      ) {
        errors.push(`${label}: nieprawidłowy klocek.`);
        continue;
      }
      if (
        Object.keys(n.inputs).length > 100 ||
        !Object.entries(n.inputs).every(
          ([k, b]) => id(k) && !['__proto__', 'constructor', 'prototype'].includes(k) && binding(b),
        )
      )
        errors.push(`${n.name}: nieprawidłowe powiązanie.`);
      if (n.config.templateInputs !== undefined && !ports(n.config.templateInputs))
        errors.push(`${n.name}: nieprawidłowe argumenty szablonów.`);
      if (n.config.inputPorts !== undefined && !ports(n.config.inputPorts))
        errors.push(`${n.name}: nieprawidłowe porty.`);
      if (
        n.kind === 'component' &&
        (!id(n.config.componentId) ||
          !Number.isInteger(n.config.revision) ||
          Number(n.config.revision) < 1)
      )
        errors.push(`${n.name}: nieprawidłowa rewizja.`);
      if (n.config.sampleResponse !== undefined && !json(n.config.sampleResponse))
        errors.push(`${n.name}: nieprawidłowa przykładowa odpowiedź.`);
      if (
        n.config.headers !== undefined &&
        (!object(n.config.headers) || !Object.values(n.config.headers).every(text))
      )
        errors.push(`${n.name}: nieprawidłowe nagłówki.`);
      if (
        n.config.query !== undefined &&
        (!object(n.config.query) || !Object.values(n.config.query).every(text))
      )
        errors.push(`${n.name}: nieprawidłowe query.`);
      for (const field of ['url', 'path', 'contractId'])
        if (n.config[field] !== undefined && !text(n.config[field]))
          errors.push(`${n.name}: nieprawidłowe pole ${field}.`);
      if (
        n.config.method !== undefined &&
        !['GET', 'HEAD', 'POST', 'PUT', 'PATCH', 'DELETE', 'OPTIONS'].includes(
          String(n.config.method),
        )
      )
        errors.push(`${n.name}: nieprawidłowa metoda HTTP.`);
      if (n.config.operation !== undefined && !isOperator(n.config.operation))
        errors.push(`${n.name}: nieprawidłowa operacja.`);
      if (
        n.config.buildType !== undefined &&
        !['object', 'array'].includes(String(n.config.buildType))
      )
        errors.push(`${n.name}: nieprawidłowy typ budowanych danych.`);
      if (
        n.config.timeoutMs !== undefined &&
        (!Number.isFinite(n.config.timeoutMs) ||
          Number(n.config.timeoutMs) <= 0 ||
          Number(n.config.timeoutMs) > 60000)
      )
        errors.push(`${n.name}: timeout poza zakresem 1–60000 ms.`);
      if (
        n.config.tolerance !== undefined &&
        (!Number.isFinite(n.config.tolerance) || Number(n.config.tolerance) < 0)
      )
        errors.push(`${n.name}: nieprawidłowa tolerancja.`);
      const predicates = [
        'equals',
        'notEquals',
        'greater',
        'less',
        'exists',
        'notEmpty',
        'contains',
        'type',
        'approximately',
      ];
      if (n.config.predicate !== undefined && !predicates.includes(String(n.config.predicate)))
        errors.push(`${n.name}: nieprawidłowe sprawdzenie.`);
      if (
        n.config.predicates !== undefined &&
        (!Array.isArray(n.config.predicates) ||
          !n.config.predicates.every(
            (p) =>
              object(p) &&
              id(p.input) &&
              id(p.expected) &&
              predicates.includes(String(p.predicate)) &&
              (p.path === undefined || text(p.path)),
          ))
      )
        errors.push(`${n.name}: nieprawidłowa lista warunków.`);
      if (n.config.match !== undefined && !['all', 'any'].includes(String(n.config.match)))
        errors.push(`${n.name}: nieprawidłowe łączenie warunków.`);
    }
    if (
      !g.edges.every(
        (e) =>
          object(e) &&
          id(e.id) &&
          id(e.source) &&
          id(e.target) &&
          (e.branch === undefined || ['true', 'false'].includes(String(e.branch))),
      )
    )
      errors.push(`${label}: nieprawidłowe połączenie.`);
    return !errors.length;
  };
  for (const s of value.scenarios) {
    if (
      !object(s) ||
      !id(s.id) ||
      !text(s.name) ||
      !text(s.description) ||
      !object(s.parameters) ||
      !json(s.parameters) ||
      !['stop', 'continue'].includes(String(s.errorPolicy))
    ) {
      errors.push('Nieprawidłowy scenariusz.');
      continue;
    }
    structuralGraph(s.graph, s.name);
    structuralGraph(s.cleanup, `${s.name}: sprzątanie`);
  }
  for (const c of value.components) {
    if (
      !object(c) ||
      !id(c.id) ||
      !text(c.name) ||
      !text(c.description) ||
      !Number.isInteger(c.revision) ||
      Number(c.revision) < 1 ||
      !ports(c.inputs) ||
      !ports(c.outputs, true)
    ) {
      errors.push('Nieprawidłowy komponent lub port.');
      continue;
    }
    structuralGraph(c.graph, c.name);
    if (
      c.action !== undefined &&
      (!object(c.action) ||
        !id(c.action.input) ||
        !(c.inputs as Port[]).some((p) => p.name === (c.action as { input: string }).input))
    )
      errors.push(`${c.name}: nieprawidłowa akcja.`);
  }
  if (errors.length) return [...new Set(errors)];
  const p = value as unknown as Project;
  if (new Set(p.scenarios.map((s) => s.id)).size !== p.scenarios.length)
    errors.push('Powtórzone ID scenariusza.');
  if (new Set(p.components.map((c) => `${c.id}@${c.revision}`)).size !== p.components.length)
    errors.push('Powtórzona rewizja komponentu.');
  const checkBinding = (b: Binding, nodes: TestNode[], label: string, parameters?: string[]) => {
    if (b.kind === 'expression') {
      Object.values(b.inputs).forEach((child) => checkBinding(child, nodes, label, parameters));
      return;
    }
    if (b.kind === 'reference') {
      const source = nodes.find((n) => n.id === b.nodeId);
      if (!source) errors.push(`${label}: brak klocka źródłowego.`);
      else if (!outputPorts(source, p).some((o) => o.name === b.port))
        errors.push(`${label}: brak wyjścia ${b.port}.`);
    }
    if (b.kind === 'parameter' && parameters && !parameters.includes(b.name))
      errors.push(`${label}: brak parametru ${b.name}.`);
  };
  function graph(g: Graph, label: string, external: TestNode[] = [], parameters?: string[]) {
    const all = [...external, ...g.nodes],
      ids = new Set(all.map((n) => n.id));
    if (ids.size !== all.length) errors.push(`${label}: powtórzone ID klocka.`);
    for (const n of g.nodes) {
      if (n.kind === 'component' && !definition(p, n))
        errors.push(`${n.name}: brak rewizji komponentu.`);
      for (const b of Object.values(n.inputs)) checkBinding(b, all, n.name, parameters);
    }
    if (g.nodes.filter((n) => n.kind === 'trigger').length > 1)
      errors.push(`${label}: dozwolony jest jeden Start.`);
    for (const n of g.nodes.filter((n) => n.kind === 'trigger'))
      if (Object.keys(n.inputs).length || g.edges.some((e) => e.target === n.id))
        errors.push(`${label}: Start nie może mieć wejść.`);
    for (const e of g.edges) {
      const source = all.find((n) => n.id === e.source);
      if (!g.nodes.some((n) => n.id === e.target) || !ids.has(e.source))
        errors.push(`${label}: połączenie z brakującym klockiem.`);
      if ((source?.kind === 'if') !== (e.branch !== undefined))
        errors.push(`${label}: wyjście IF musi wskazywać gałąź tak albo nie.`);
    }
    if (hasCycle(g)) errors.push(`${label}: graf zawiera cykl.`);
  }
  for (const s of p.scenarios) {
    graph(s.graph, s.name, [], Object.keys(s.parameters));
    graph(s.cleanup, `${s.name}: sprzątanie`, s.graph.nodes, Object.keys(s.parameters));
  }
  for (const c of p.components) {
    graph(
      c.graph,
      c.name,
      [],
      c.inputs.map((p) => p.name),
    );
    for (const output of c.outputs)
      checkBinding(
        output.binding,
        c.graph.nodes,
        `${c.name}: ${output.name}`,
        c.inputs.map((p) => p.name),
      );
  }
  const done = new Set<string>(),
    visiting = new Set<string>();
  const definitions = new Map(p.components.map((c) => [`${c.id}@${c.revision}`, c]));
  const visit = (key: string): void => {
    if (done.has(key)) return;
    if (visiting.has(key)) {
      errors.push('Rekurencyjna zależność komponentów.');
      return;
    }
    visiting.add(key);
    const c = definitions.get(key);
    if (c)
      for (const n of c.graph.nodes)
        if (n.kind === 'component') visit(`${n.config.componentId}@${n.config.revision}`);
    visiting.delete(key);
    done.add(key);
  };
  if (!errors.length) for (const c of p.components) visit(`${c.id}@${c.revision}`);
  return [...new Set(errors)];
}

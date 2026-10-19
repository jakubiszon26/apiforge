import type { Binding, TestNode } from './model';

export const VARIABLE_MIME = 'application/apiforge-variable';
export interface VariableBlock {
  label: string;
  binding: Extract<Binding, { kind: 'reference' | 'parameter' | 'environment' }>;
}
export interface TextSelection {
  value: string;
  start: number;
  end: number;
}
export function readVariable(data: Pick<DataTransfer, 'getData'>): VariableBlock | undefined {
  try {
    const value = JSON.parse(data.getData(VARIABLE_MIME));
    const text = (value: unknown): value is string =>
      typeof value === 'string' &&
      value.length > 0 &&
      value.length <= 200 &&
      !['__proto__', 'constructor', 'prototype'].includes(value);
    if (!text(value.label) || !value.binding) return;
    const b = value.binding;
    if (b.kind === 'parameter' || b.kind === 'environment')
      return text(b.name)
        ? { label: value.label, binding: { kind: b.kind, name: b.name } }
        : undefined;
    if (
      b.kind === 'reference' &&
      text(b.nodeId) &&
      text(b.port) &&
      typeof b.path === 'string' &&
      b.path.length <= 100000
    )
      return {
        label: value.label,
        binding: { kind: 'reference', nodeId: b.nodeId, port: b.port, path: b.path },
      };
  } catch {
    /* Other drag payloads are not variable blocks. */
  }
}
export function writeVariable(
  data: Pick<DataTransfer, 'setData' | 'effectAllowed'>,
  variable: VariableBlock,
) {
  // Drag payloads contain names and bindings only, never secret values.
  data.setData(VARIABLE_MIME, JSON.stringify(variable));
  data.setData('text/plain', `{{${variable.label}}}`);
  data.effectAllowed = 'copy';
}
export function insertVariableToken(selection: TextSelection, name: string): string {
  return (
    selection.value.slice(0, selection.start) + `{{${name}}}` + selection.value.slice(selection.end)
  );
}
/** Text templates use named inputs so graph dependencies remain explicit and exportable. */
export function bindVariableText(
  node: TestNode,
  variable: VariableBlock,
  selection: TextSelection,
  write: (node: TestNode, text: string) => TestNode,
): TestNode {
  const same = (binding: Binding) => JSON.stringify(binding) === JSON.stringify(variable.binding);
  let name = Object.entries(node.inputs).find(([, binding]) => same(binding))?.[0];
  let next = node;
  if (!name) {
    const base =
      variable.label
        .normalize('NFKD')
        .replace(/[\u0300-\u036f]/g, '')
        .replace(/[^a-zA-Z0-9_]/g, '_')
        .slice(0, 60) || 'zmienna';
    name = /^[a-zA-Z_]/.test(base) ? base : `v_${base}`;
    const initial = name;
    let suffix = 2;
    const declared = new Set([
      'body',
      'value',
      'actual',
      'expected',
      'left',
      'right',
      ...Object.keys(node.inputs),
      ...(node.config.inputPorts ?? []).map((port) => port.name),
      ...(node.config.templateInputs ?? []).map((port) => port.name),
    ]);
    while (declared.has(name)) name = `${initial}_${suffix++}`;
    next = {
      ...node,
      inputs: { ...node.inputs, [name]: variable.binding },
      config: {
        ...node.config,
        templateInputs: [
          ...(node.config.templateInputs ?? []),
          { name, label: variable.label, type: 'any', required: true, exposed: false },
        ],
      },
    };
  }
  return write(next, insertVariableToken(selection, name));
}

/** Removed tokens must not leave unused execution dependencies behind. */
export function pruneTemplateInputs(node: TestNode): TestNode {
  const declared = node.config.templateInputs;
  if (!declared?.length) return node;
  const texts = [
    node.config.url,
    node.config.path,
    ...Object.values(node.config.headers ?? {}),
    ...Object.values(node.config.query ?? {}),
    ...(node.config.predicates ?? []).map((predicate) => predicate.path),
  ];
  const used = new Set(
    texts.flatMap((text) =>
      [...(text ?? '').matchAll(/\{\{\s*([^{}]+?)\s*\}\}/g)].map((match) => match[1]),
    ),
  );
  const obsolete = new Set(
    declared.filter((port) => !used.has(port.name)).map((port) => port.name),
  );
  if (!obsolete.size) return node;
  return {
    ...node,
    inputs: Object.fromEntries(Object.entries(node.inputs).filter(([name]) => !obsolete.has(name))),
    config: { ...node.config, templateInputs: declared.filter((port) => !obsolete.has(port.name)) },
  };
}

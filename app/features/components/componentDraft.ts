import { operators } from '../../../shared/operators';
import { bindingLeaves, replaceBindingAt, mapBinding } from '../../../shared/bindings';
import type {
  Binding,
  ComponentDefinition,
  Graph,
  Json,
  Port,
  Project,
  ValueType,
} from '../../../shared/model';
import { uid } from '../../../shared/model';
import { inputPorts, outputPorts } from '../../../shared/graph';

export interface InputRow {
  key: string;
  nodeId: string;
  input: string;
  label: string;
  enabled: boolean;
  mandatory: boolean;
  name: string;
  type: ValueType;
  contractId?: string;
  exposed: boolean;
  required: boolean;
  original: Binding | undefined;
  targets: { nodeId: string; input: string; path?: string[] }[];
  defaultValue?: Json;
  publicLabel?: string;
}
export interface OutputRow {
  key: string;
  enabled: boolean;
  name: string;
  type: ValueType;
  contractId?: string;
  nodeId: string;
  port: string;
  path: string;
  originalBinding?: Binding;
  label?: string;
}
export interface ComponentDraft {
  name: string;
  description: string;
  inputs: InputRow[];
  outputs: OutputRow[];
  action: string;
  actionType: ValueType;
  actionContract: string;
}

export function initializeInputs(
  graph: Graph,
  selectedIds: string[],
  project: Project,
  initialDefinition?: ComponentDefinition,
): InputRow[] {
  const used = new Set<string>();
  const rows: InputRow[] = [];
  for (const node of graph.nodes.filter((n) => selectedIds.includes(n.id))) {
    const declared = inputPorts(node, project);
    for (const p of [
      ...declared,
      ...Object.keys(node.inputs)
        .filter((k) => !declared.some((p) => p.name === k))
        .map((name): Port => ({ name, type: 'any' })),
    ]) {
      const root = node.inputs[p.name];
      const leaves =
        root?.kind === 'expression' ? bindingLeaves(root) : [{ binding: root, path: [] }];
      for (const { binding: original, path } of leaves) {
        const target = { nodeId: node.id, input: p.name, path };
        // Public leaf contracts come from their immediate operator slot, not the outer result.
        let leafPort = p;
        let cursor = root;
        for (const name of path) {
          if (cursor?.kind !== 'expression') break;
          leafPort =
            operators[cursor.operator].inputs.find((port) => port.name === name) ?? leafPort;
          cursor = cursor.inputs[name];
        }
        if (
          (original?.kind === 'reference' && selectedIds.includes(original.nodeId)) ||
          original?.kind === 'environment'
        )
          continue;
        const existing =
          original?.kind === 'parameter'
            ? rows.find(
                (r) => r.original?.kind === 'parameter' && r.original.name === original.name,
              )
            : undefined;
        if (existing) {
          existing.targets.push(target);
          continue;
        }
        const publicPort =
          original?.kind === 'parameter'
            ? initialDefinition?.inputs.find((i) => i.name === original.name)
            : undefined;
        let proposed = original?.kind === 'parameter' ? original.name : [p.name, ...path].join('_');
        let i = 2;
        while (used.has(proposed)) proposed = `${p.name}_${i++}`;
        used.add(proposed);
        const mandatory =
          original?.kind === 'reference' ||
          original?.kind === 'parameter' ||
          (!original && Boolean(p.required));
        rows.push({
          key: `${node.id}:${p.name}:${path.join('/')}`,
          nodeId: node.id,
          input: p.name,
          label: `${node.name} → ${p.label ?? p.name}${path.length ? ` → ${path.map((name) => (name === 'left' ? 'A' : 'B')).join(' → ')}` : ''}`,
          enabled: mandatory || (!path.length && Boolean(p.exposed)),
          mandatory,
          name: proposed,
          type: publicPort?.type ?? leafPort.type,
          contractId: publicPort?.contractId ?? leafPort.contractId,
          publicLabel: publicPort?.label,
          exposed: publicPort?.exposed ?? p.exposed ?? true,
          required: publicPort?.required ?? leafPort.required ?? true,
          original,
          targets: [target],
          defaultValue:
            publicPort?.defaultValue !== undefined
              ? publicPort.defaultValue
              : original?.kind === 'literal'
                ? original.value
                : undefined,
        });
      }
    }
  }
  // An argument may be returned directly, or kept for compatibility, without feeding a node.
  for (const port of initialDefinition?.inputs ?? [])
    if (!rows.some((r) => r.original?.kind === 'parameter' && r.original.name === port.name))
      rows.push({
        key: `public:${port.name}`,
        nodeId: '',
        input: port.name,
        label: `Publiczny argument → ${port.label ?? port.name}`,
        enabled: true,
        mandatory: true,
        name: port.name,
        type: port.type,
        contractId: port.contractId,
        exposed: port.exposed ?? true,
        required: port.required ?? false,
        original: { kind: 'parameter', name: port.name },
        targets: [],
        defaultValue: port.defaultValue,
        publicLabel: port.label,
      });
  return structuredClone(rows);
}
export function initializeOutputs(
  graph: Graph,
  selectedIds: string[],
  project: Project,
  initialDefinition?: ComponentDefinition,
): OutputRow[] {
  const nodes = graph.nodes.filter((n) => selectedIds.includes(n.id));
  return structuredClone(
    initialDefinition
      ? initialDefinition.outputs.map((p, i) => ({
          key: `output-${i}`,
          enabled: true,
          name: p.name,
          label: p.label,
          type: p.type,
          contractId: p.contractId,
          nodeId: p.binding.kind === 'reference' ? p.binding.nodeId : '',
          port: p.binding.kind === 'reference' ? p.binding.port : '',
          path: p.binding.kind === 'reference' ? p.binding.path : '',
          originalBinding: p.binding,
        }))
      : nodes.flatMap((node, index) =>
          outputPorts(node, project).map((p, i) => ({
            key: `${node.id}:${p.name}`,
            enabled: index === nodes.length - 1 && i === 0,
            name: p.name,
            label: p.label,
            type: p.type,
            contractId: p.contractId,
            nodeId: node.id,
            port: p.name,
            path: '',
          })),
        ),
  );
}
export function validateDraft(
  graph: Graph,
  selectedIds: string[],
  draft: ComponentDraft,
): string[] {
  const nodes = graph.nodes.filter((n) => selectedIds.includes(n.id));
  const activeInputs = draft.inputs.filter((i) => i.enabled);
  const activeOutputs = draft.outputs.filter((o) => o.enabled);
  const errors: string[] = [];
  if (!nodes.length) errors.push('Zaznacz co najmniej jeden klocek.');
  if (!draft.name.trim()) errors.push('Podaj nazwę komponentu.');
  if (!activeOutputs.length) errors.push('Wybierz przynajmniej jedno wyjście.');
  if (draft.inputs.some((i) => i.mandatory && !i.enabled))
    errors.push('Powiązania spoza komponentu muszą być publicznymi argumentami.');
  if (
    activeOutputs.some(
      (o) =>
        (!o.originalBinding || o.originalBinding.kind === 'reference') &&
        !nodes.some((n) => n.id === o.nodeId),
    )
  )
    errors.push('Wyjście odwołuje się do usuniętego kroku. Wybierz nowe źródło.');
  for (const [label, items] of [
    ['Argumenty', activeInputs],
    ['Wyjścia', activeOutputs],
  ] as const) {
    if (items.some((p) => !p.name.trim())) errors.push(`${label}: nazwy nie mogą być puste.`);
    if (new Set(items.map((p) => p.name.trim())).size !== items.length)
      errors.push(`${label}: nazwy muszą być unikalne.`);
    if (items.some((p) => !/^[_a-zA-Z][\w-]*$/.test(p.name)))
      errors.push(`${label}: użyj liter bez polskich znaków, cyfr, _ lub -; zacznij od litery.`);
  }
  if (draft.action && !activeInputs.some((i) => i.key === draft.action))
    errors.push('Wybierz dostępny argument akcji.');
  return errors;
}
/** Replace public input targets with parameters; clone the graph to preserve the scenario. */
export function materializeComponent(
  graph: Graph,
  selectedIds: string[],
  draft: ComponentDraft,
  initialDefinition?: ComponentDefinition,
): ComponentDefinition {
  const errors = validateDraft(graph, selectedIds, draft);
  if (errors.length) throw new Error(errors.join('\n'));
  const activeInputs = draft.inputs.filter((i) => i.enabled);
  const activeOutputs = draft.outputs.filter((o) => o.enabled);
  const nodes = structuredClone(graph.nodes.filter((n) => selectedIds.includes(n.id))).map(
    (node) => {
      const mapped = { ...node.inputs };
      for (const row of activeInputs)
        for (const target of row.targets)
          if (target.nodeId === node.id)
            mapped[target.input] = target.path?.length
              ? replaceBindingAt(mapped[target.input], target.path, {
                  kind: 'parameter',
                  name: row.name.trim(),
                })
              : { kind: 'parameter', name: row.name.trim() };
      return { ...node, inputs: mapped };
    },
  );
  const actionRow = activeInputs.find((i) => i.key === draft.action);
  return structuredClone({
    id: initialDefinition?.id ?? uid('component'),
    revision: initialDefinition?.revision ?? 1,
    name: draft.name.trim(),
    description: draft.description.trim(),
    inputs: activeInputs.map((row) => ({
      name: row.name.trim(),
      label: row.publicLabel,
      type: row.type,
      contractId: row.contractId || undefined,
      required: row.required,
      exposed: row.exposed,
      ...(row.defaultValue !== undefined ? { defaultValue: row.defaultValue } : {}),
    })),
    outputs: activeOutputs.map((row) => {
      let binding: Binding =
        row.originalBinding && row.originalBinding.kind !== 'reference'
          ? row.originalBinding
          : { kind: 'reference', nodeId: row.nodeId, port: row.port, path: row.path };
      binding = mapBinding(binding, (leaf) => {
        if (leaf.kind !== 'parameter') return leaf;
        const renamed = activeInputs.find(
          (i) => i.original?.kind === 'parameter' && i.original.name === leaf.name,
        );
        return renamed ? { kind: 'parameter', name: renamed.name.trim() } : leaf;
      })!;
      return {
        name: row.name.trim(),
        label: row.label,
        type: row.type,
        contractId: row.contractId || undefined,
        binding,
      };
    }),
    graph: {
      nodes,
      edges: graph.edges.filter(
        (e) => selectedIds.includes(e.source) && selectedIds.includes(e.target),
      ),
    },
    ...(actionRow
      ? {
          action: {
            input: actionRow.name.trim(),
            type: draft.actionType,
            contractId: draft.actionContract || undefined,
          },
        }
      : {}),
  });
}

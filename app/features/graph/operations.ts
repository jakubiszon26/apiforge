import { references, mapBinding, bindingEdgeId } from '../../../shared/bindings';
import type { Graph, Project } from '../../../shared/model';
import { reference, uid } from '../../../shared/model';
import { bindInput, compatible, hasCycle, inputPorts, outputPorts } from '../../../shared/graph';

export function connectGraph(
  graph: Graph,
  project: Project,
  source: string,
  sourcePort: string,
  target: string,
  targetInput: string,
): { graph?: Graph; error?: string } {
  if (source === target) return { error: 'Klocek nie może być połączony sam ze sobą.' };
  const from = graph.nodes.find((n) => n.id === source),
    to = graph.nodes.find((n) => n.id === target);
  if (!from || !to) return { error: 'Nie znaleziono klocka.' };
  let next: Graph;
  if (['__order', '__true', '__false'].includes(sourcePort) && targetInput === '__order') {
    if (to.kind === 'trigger') return { error: 'Start nie może mieć poprzednika.' };
    const branch =
      sourcePort === '__true' ? 'true' : sourcePort === '__false' ? 'false' : undefined;
    if ((from.kind === 'if') !== (branch !== undefined))
      return { error: 'Wybierz wyjście IF: tak albo nie.' };
    if (graph.edges.some((e) => e.source === source && e.target === target && e.branch === branch))
      return { error: 'Ta zależność już istnieje.' };
    next = {
      ...graph,
      edges: [...graph.edges, { id: uid('order'), source, target, ...(branch ? { branch } : {}) }],
    };
  } else {
    const out = outputPorts(from, project).find((p) => p.name === sourcePort),
      input = inputPorts(to, project).find((p) => p.name === targetInput);
    if (!out || !input || !compatible(out, input))
      return { error: 'Te porty mają niezgodne typy danych.' };
    if (to.inputs[targetInput] && references(to.inputs[targetInput]).length)
      return { error: 'To wejście ma już źródło. Usuń jego połączenie przed przypisaniem nowego.' };
    next = bindInput(graph, target, targetInput, reference(source, sourcePort));
  }
  if (hasCycle(next))
    return { error: 'To połączenie utworzyłoby cykl. Test musi mieć jednoznaczną kolejność.' };
  return { graph: next };
}

/** Freeze the selection now: later edits to the originals must not change the clipboard. */
export function copySelection(graph: Graph, selected: string[]): Graph {
  const ids = new Set(selected);
  return structuredClone({
    nodes: graph.nodes.filter((node) => ids.has(node.id)),
    edges: graph.edges.filter((edge) => ids.has(edge.source) && ids.has(edge.target)),
  });
}

type PasteResult = { graph: Graph; ids: string[]; error?: string };

export function pasteSelection(
  graph: Graph,
  fragment: Graph,
  offset = { x: 40, y: 50 },
  externalIds: string[] = [],
): PasteResult {
  if (
    fragment.nodes.some((node) => node.kind === 'trigger') &&
    graph.nodes.some((node) => node.kind === 'trigger')
  )
    return {
      graph,
      ids: [],
      error: 'Plansza ma już Start. Zaznacz klocki bez Startu, aby je skopiować.',
    };
  const mapping = new Map(fragment.nodes.map((node) => [node.id, uid()]));
  const available = new Set([
    ...graph.nodes.map((node) => node.id),
    ...externalIds,
    ...mapping.keys(),
  ]);
  if (
    fragment.nodes.some((node) =>
      Object.values(node.inputs)
        .flatMap(references)
        .some((ref) => !available.has(ref.nodeId)),
    )
  )
    return {
      graph,
      ids: [],
      error:
        'Brakuje źródła danych skopiowanego klocka. Przywróć je albo skopiuj również klocek źródłowy.',
    };
  // Give the copies fresh identities; reconnect internal references, including nested operators.
  // References outside the selection keep their original producers on this board.
  const nodes = structuredClone(fragment.nodes).map((node) => ({
    ...node,
    id: mapping.get(node.id)!,
    name: `${node.name} — kopia`,
    position: { x: node.position.x + offset.x, y: node.position.y + offset.y },
    inputs: Object.fromEntries(
      Object.entries(node.inputs).map(([key, binding]) => [
        key,
        mapBinding(binding, (leaf) =>
          leaf.kind === 'reference'
            ? { ...leaf, nodeId: mapping.get(leaf.nodeId) ?? leaf.nodeId }
            : leaf,
        )!,
      ]),
    ),
  }));
  const edges = fragment.edges.map((edge) => ({
    ...structuredClone(edge),
    id: uid('order'),
    source: mapping.get(edge.source)!,
    target: mapping.get(edge.target)!,
  }));
  return {
    graph: { nodes: [...graph.nodes, ...nodes], edges: [...graph.edges, ...edges] },
    ids: nodes.map((node) => node.id),
  };
}

export function duplicateSelection(graph: Graph, selected: string[]): PasteResult {
  return pasteSelection(graph, copySelection(graph, selected));
}

export function removeConnections(graph: Graph, edgeIds: string[]): Graph {
  return {
    edges: graph.edges.filter((e) => !edgeIds.includes(e.id)),
    nodes: graph.nodes.map((n) => ({
      ...n,
      inputs: Object.fromEntries(
        Object.entries(n.inputs).flatMap(([name, b]) => {
          const next = mapBinding(b, (leaf, path) =>
            edgeIds.includes(bindingEdgeId(n.id, name, path)) ? undefined : leaf,
          );
          return next ? [[name, next]] : [];
        }),
      ),
    })),
  };
}

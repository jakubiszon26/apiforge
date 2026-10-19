import { references } from '../../shared/bindings';
import type { Graph } from '../../shared/model';

/** Build dependency indexes once, rather than walking the whole graph for each ready step. */
export function planExecution(graph: Graph) {
  const nodes = new Map(graph.nodes.map((node) => [node.id, node]));
  const dependencies = new Map(
    graph.nodes.map((node) => [
      node.id,
      new Set(
        Object.values(node.inputs)
          .flatMap(references)
          .map((ref) => ref.nodeId),
      ),
    ]),
  );
  for (const edge of graph.edges) dependencies.get(edge.target)?.add(edge.source);
  const successors = new Map<string, Set<string>>();
  for (const [target, sources] of dependencies)
    for (const source of sources) {
      if (!successors.has(source)) successors.set(source, new Set());
      successors.get(source)!.add(target);
    }
  const trigger = graph.nodes.find((node) => node.kind === 'trigger');
  // Undefined means a legacy graph: execute all roots, still awaiting every operation.
  const reachable = trigger ? new Set([trigger.id]) : undefined;
  const queue = trigger ? [trigger.id] : [];
  for (let index = 0; index < queue.length; index++) {
    const id = queue[index];
    const add = (next: string) => {
      if (!reachable!.has(next)) {
        reachable!.add(next);
        queue.push(next);
      }
    };
    for (const next of successors.get(id) ?? []) add(next);
    // Value blocks are arguments. External HTTP producers must themselves be on
    // the path from Start; a reference must never silently enable another request.
    for (const source of dependencies.get(id) ?? []) {
      const node = nodes.get(source);
      if (node && ['variable', 'select', 'build', 'formula'].includes(node.kind)) add(source);
    }
  }
  return { dependencies, reachable };
}

import type { Graph, Project } from '../../../shared/model';
import { reference } from '../../../shared/model';
import { createNode, hasCycle, outputPorts } from '../../../shared/graph';

/** Keep newly created variable blocks clear of existing node hit targets. */
export function variablePosition(graph: Graph, y = 100) {
  return {
    x: graph.nodes.length ? Math.max(...graph.nodes.map((node) => node.position.x)) + 340 : 100,
    y,
  };
}

/** Capture a response binding, never the value observed in a previous run. */
export function assignResponseVariable(
  graph: Graph,
  project: Project,
  sourceId: string,
  port: string,
  path: string,
  name: string,
  targetId?: string,
): { graph?: Graph; variableId?: string; error?: string } {
  const source = graph.nodes.find((node) => node.id === sourceId);
  if (!source || !outputPorts(source, project).some((output) => output.name === port))
    return { error: 'Źródło odpowiedzi nie jest już dostępne.' };
  if (!name.trim()) return { error: 'Podaj nazwę zmiennej.' };
  const target = targetId
    ? graph.nodes.find((node) => node.id === targetId)
    : createNode('variable', variablePosition(graph, source.position.y));
  if (!target || target.kind !== 'variable') return { error: 'Wybierz istniejącą zmienną.' };
  const variable = {
    ...target,
    name: name.trim(),
    inputs: { ...target.inputs, value: reference(source.id, port, path) },
  };
  const next = {
    ...graph,
    nodes: targetId
      ? graph.nodes.map((node) => (node.id === targetId ? variable : node))
      : [...graph.nodes, variable],
  };
  if (hasCycle(next))
    return {
      error:
        'To przypisanie tworzy cykl: request nie może korzystać ze zmiennej pobieranej z jego własnej odpowiedzi.',
    };
  return { graph: next, variableId: variable.id };
}

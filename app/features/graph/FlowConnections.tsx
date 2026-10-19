import { useState } from 'react';
import type { Graph, Project, TestNode } from '../../../shared/model';
import { connectGraph, removeConnections } from './operations';

/** A keyboard-accessible alternative to dragging the control wires. */
export function FlowConnections({
  graph,
  project,
  node,
  onChange,
}: {
  graph: Graph;
  project: Project;
  node: TestNode;
  onChange: (graph: Graph) => void;
}) {
  const [error, setError] = useState('');
  return (
    <section aria-label="Kolejność wykonania">
      <div className="af-flow-editor nodrag nowheel">
        <strong>{node.name} → następny krok</strong>
        {(node.kind === 'if' ? ['true', 'false'] : ['order']).map((port) => (
          <label key={port}>
            {port === 'true' ? 'Tak' : port === 'false' ? 'Nie' : 'Dalej'}
            <select
              aria-label={`Następny krok: ${port === 'true' ? 'tak' : port === 'false' ? 'nie' : 'dalej'}`}
              value=""
              onChange={(event) => {
                const result = connectGraph(
                  graph,
                  project,
                  node.id,
                  `__${port}`,
                  event.target.value,
                  '__order',
                );
                if (result.graph) {
                  onChange(result.graph);
                  setError('');
                } else setError(result.error ?? 'Nieprawidłowe połączenie.');
              }}
            >
              <option value="">Połącz z…</option>
              {graph.nodes
                .filter((n) => n.id !== node.id && n.kind !== 'trigger')
                .map((n) => (
                  <option key={n.id} value={n.id}>
                    {n.name}
                  </option>
                ))}
            </select>
          </label>
        ))}
        {graph.edges
          .filter((e) => e.source === node.id)
          .map((edge) => (
            <div key={edge.id}>
              {edge.branch === 'true' ? 'Tak' : edge.branch === 'false' ? 'Nie' : 'Dalej'} →{' '}
              {graph.nodes.find((n) => n.id === edge.target)?.name}
              <button
                aria-label="Usuń połączenie kolejności"
                onClick={() => onChange(removeConnections(graph, [edge.id]))}
              >
                ×
              </button>
            </div>
          ))}
      </div>
      {error && (
        <p role="alert" className="af-hint">
          {error}
        </p>
      )}
    </section>
  );
}

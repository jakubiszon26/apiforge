import { useState } from 'react';
import type { Binding, Graph, Project, TestNode } from '../../../shared/model';
import { reference } from '../../../shared/model';
import { bindInput, hasCycle, outputPorts } from '../../../shared/graph';
import { ResponseFieldDialog } from '../inspector/ResponseFieldPicker';
import { useResponseSource } from '../inspector/responseData';

/** Pick a source and a field together instead of manually configuring three binding controls. */
export function ResponseBindingPicker({
  node,
  graph,
  project,
  onChange,
  onClose,
}: {
  node: TestNode;
  graph: Graph;
  project: Project;
  onChange: (binding: Binding) => void;
  onClose: () => void;
}) {
  const sources = graph.nodes.filter(
    (source) =>
      (source.kind === 'request' || source.kind === 'component') &&
      source.id !== node.id &&
      !hasCycle(
        bindInput(
          graph,
          node.id,
          'value',
          reference(source.id, outputPorts(source, project)[0]?.name ?? 'body'),
        ),
      ),
  );
  const binding = node.inputs.value;
  const initial =
    sources.find((source) => binding?.kind === 'reference' && source.id === binding.nodeId) ??
    sources[0];
  const [sourceId, setSourceId] = useState(initial?.id ?? '');
  const [port, setPort] = useState(
    binding?.kind === 'reference' && binding.nodeId === initial?.id
      ? binding.port
      : initial
        ? (outputPorts(initial, project)[0]?.name ?? 'body')
        : 'body',
  );
  const source = sources.find((source) => source.id === sourceId);
  const outputs = source ? outputPorts(source, project) : [];
  const { sample, origin } = useResponseSource(source, port, project);
  return (
    <ResponseFieldDialog
      sample={sample}
      origin={origin}
      value={
        binding?.kind === 'reference' && binding.nodeId === sourceId && binding.port === port
          ? binding.path
          : ''
      }
      port={port}
      onClose={onClose}
      onChange={(path) => {
        if (source) onChange(reference(source.id, port, path));
      }}
      sourceControl={
        <>
          <label className="af-field">
            Request lub komponent
            <select
              aria-label="Źródło odpowiedzi"
              value={sourceId}
              onChange={(event) => {
                const source = sources.find((source) => source.id === event.target.value)!;
                setSourceId(source.id);
                setPort(outputPorts(source, project)[0]?.name ?? 'body');
              }}
            >
              {!sources.length && <option value="">Brak dostępnego requestu</option>}
              {sources.map((source) => (
                <option key={source.id} value={source.id}>
                  {source.name}
                </option>
              ))}
            </select>
          </label>
          <label className="af-field">
            Wyjście
            <select
              aria-label="Wyjście odpowiedzi"
              value={port}
              onChange={(event) => setPort(event.target.value)}
            >
              {outputs.map((port) => (
                <option key={port.name} value={port.name}>
                  {port.label ?? port.name}
                </option>
              ))}
            </select>
          </label>
          {!sources.length && (
            <p className="af-hint">
              Dodaj request niezależny od tej zmiennej. Pole możesz wybrać po wykonaniu requestu lub
              dodaniu przykładowej odpowiedzi.
            </p>
          )}
        </>
      }
    />
  );
}

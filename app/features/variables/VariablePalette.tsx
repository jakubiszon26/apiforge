import type { Graph, Json, Project } from '../../../shared/model';
import { writeVariable, type VariableBlock } from '../../../shared/variables';
import './variables.css';
export function VariablePalette({
  graph,
  project,
  parameters,
  onAdd,
  onSelect,
  search,
}: {
  graph: Graph;
  project: Project;
  parameters: Record<string, Json>;
  onAdd: () => void;
  onSelect: (id: string) => void;
  search: string;
}) {
  const blocks: (VariableBlock & { group: string; nodeId?: string })[] = [
    ...graph.nodes
      .filter((node) => node.kind === 'variable')
      .map((node) => ({
        label: node.name,
        binding: { kind: 'reference' as const, nodeId: node.id, port: 'value', path: '' },
        group: 'Zmienna',
        nodeId: node.id,
      })),
    ...Object.keys(parameters).map((name) => ({
      label: name,
      binding: { kind: 'parameter' as const, name },
      group: 'Parametr',
    })),
    ...[
      'baseUrl',
      ...Object.keys(project.environment.variables),
      ...project.environment.secretNames,
    ].map((name) => ({
      label: name,
      binding: { kind: 'environment' as const, name },
      group: project.environment.secretNames.includes(name) ? 'Sekret' : 'Środowisko',
    })),
  ];
  return (
    <section className="af-variable-palette" aria-label="Bloczki zmiennych">
      <div className="af-library-category">
        ZMIENNE{' '}
        <button type="button" title="Dodaj zmienną" aria-label="Dodaj zmienną" onClick={onAdd}>
          +
        </button>
      </div>
      <p>Przeciągnij bloczek do pola tekstowego lub argumentu.</p>
      {blocks
        .filter((block) =>
          block.label.toLocaleLowerCase('pl').includes(search.toLocaleLowerCase('pl')),
        )
        .map((block) => (
          <button
            type="button"
            draggable
            key={`${block.binding.kind}:${block.nodeId ?? block.label}`}
            className="af-variable-chip"
            aria-label={`Zmienna ${block.label}`}
            onClick={() => {
              if (block.nodeId) onSelect(block.nodeId);
            }}
            onDragStart={(event) => writeVariable(event.dataTransfer, block)}
            title={`${block.group} · przeciągnij do pola`}
          >
            <strong>{block.label}</strong>
            <small>{block.group}</small>
          </button>
        ))}
    </section>
  );
}

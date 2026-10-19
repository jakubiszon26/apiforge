import { useState } from 'react';
import type { ComponentDefinition, Graph, Project, ValueType } from '../../../shared/model';
import { uid } from '../../../shared/model';
import { outputPorts } from '../../../shared/graph';
import { PathPicker, sampleOutput, TypeSelect } from '../inspector/fields';
import '../inspector/inspector.css';
import { useModalFocus } from '../inspector/useModalFocus';

export interface ComponentBuilderProps {
  graph: Graph;
  selectedIds: string[];
  project: Project;
  onSave: (definition: ComponentDefinition) => string | void;
  onClose: () => void;
  initialDefinition?: ComponentDefinition;
}
import {
  initializeInputs,
  initializeOutputs,
  materializeComponent,
  validateDraft,
} from './componentDraft';
import type { InputRow, OutputRow } from './componentDraft';
export function ComponentBuilder({
  graph,
  selectedIds,
  project,
  onSave,
  onClose,
  initialDefinition,
}: ComponentBuilderProps) {
  const [saveError, setSaveError] = useState('');
  const modalRef = useModalFocus<HTMLElement>(onClose);
  const nodes = graph.nodes.filter((n) => selectedIds.includes(n.id));
  const [name, setName] = useState(
    initialDefinition?.name ?? (nodes.length === 1 ? nodes[0].name : 'Nowy komponent'),
  );
  const [description, setDescription] = useState(initialDefinition?.description ?? '');
  const [inputs, setInputs] = useState<InputRow[]>(() =>
    initializeInputs(graph, selectedIds, project, initialDefinition),
  );
  const [outputs, setOutputs] = useState<OutputRow[]>(() =>
    initializeOutputs(graph, selectedIds, project, initialDefinition),
  );
  const [action, setAction] = useState(
    () => inputs.find((i) => i.name === initialDefinition?.action?.input)?.key ?? '',
  );
  const [actionType, setActionType] = useState<ValueType>(initialDefinition?.action?.type ?? 'any');
  const [actionContract, setActionContract] = useState(initialDefinition?.action?.contractId ?? '');
  const activeInputs = inputs.filter((i) => i.enabled);
  const activeOutputs = outputs.filter((o) => o.enabled);
  const draft = { name, description, inputs, outputs, action, actionType, actionContract };
  const errors = validateDraft(graph, selectedIds, draft);
  const outsideOrders = graph.edges.filter(
    (e) => selectedIds.includes(e.source) !== selectedIds.includes(e.target),
  );
  const updateInput = (key: string, patch: Partial<InputRow>) =>
    setInputs(inputs.map((row) => (row.key === key ? { ...row, ...patch } : row)));
  const updateOutput = (key: string, patch: Partial<OutputRow>) =>
    setOutputs(outputs.map((row) => (row.key === key ? { ...row, ...patch } : row)));
  const save = () => {
    if (errors.length) return;
    setSaveError(onSave(materializeComponent(graph, selectedIds, draft, initialDefinition)) || '');
  };
  return (
    <div
      className="af-modal-backdrop"
      onClick={(e) => {
        if (e.target === e.currentTarget) onClose();
      }}
    >
      <section
        ref={modalRef}
        tabIndex={-1}
        className="af-modal af-editor af-component-builder"
        role="dialog"
        aria-modal="true"
        aria-labelledby="af-component-title"
      >
        <header className="af-modal-header">
          <div>
            <span className="af-eyebrow">Biblioteka projektu · {nodes.length} kroków</span>
            <h2 id="af-component-title">
              {initialDefinition ? 'Zapisz nową rewizję' : 'Zapisz własny komponent'}
            </h2>
          </div>
          <button type="button" aria-label="Zamknij" onClick={onClose}>
            ×
          </button>
        </header>
        <div className="af-modal-body">
          <div className="af-inline">
            <label className="af-field">
              Nazwa
              <input
                data-autofocus
                value={name}
                onChange={(e) => setName(e.target.value)}
                placeholder="np. Pobierz rekord"
              />
            </label>
            <label className="af-field">
              Opis
              <input
                value={description}
                onChange={(e) => setDescription(e.target.value)}
                placeholder="Co robi ten komponent?"
              />
            </label>
          </div>
          <div className="af-section-title">1. Publiczne argumenty</div>
          <p className="af-hint">
            Wybierz wartości, które tester może zmieniać przy każdym użyciu. Dane spoza zaznaczenia
            muszą stać się argumentami. Pozostałe wartości zostaną wewnątrz komponentu.
          </p>
          {!inputs.length && (
            <p className="af-empty">Ten fragment nie wymaga publicznych argumentów.</p>
          )}
          {inputs.map((row) => (
            <div className={`af-builder-row ${row.enabled ? 'is-enabled' : ''}`} key={row.key}>
              <label className="af-check">
                <input
                  type="checkbox"
                  checked={row.enabled}
                  disabled={row.mandatory}
                  onChange={(e) => updateInput(row.key, { enabled: e.target.checked })}
                />
                {row.label}
                {row.mandatory && <span className="af-badge">Granica grafu</span>}
              </label>
              {row.enabled && (
                <>
                  <div className="af-inline">
                    <label className="af-field">
                      Nazwa argumentu
                      <input
                        value={row.name}
                        onChange={(e) => updateInput(row.key, { name: e.target.value })}
                      />
                    </label>
                    <label className="af-field">
                      Typ
                      <TypeSelect
                        value={row.type}
                        onChange={(type) => updateInput(row.key, { type })}
                      />
                    </label>
                    <label className="af-field">
                      Kontrakt
                      <select
                        value={row.contractId ?? ''}
                        onChange={(e) => updateInput(row.key, { contractId: e.target.value })}
                      >
                        <option value="">Bez kontraktu</option>
                        {project.contracts.map((c) => (
                          <option key={c.id} value={c.id}>
                            {c.name}
                          </option>
                        ))}
                      </select>
                    </label>
                  </div>
                  <div className="af-inline">
                    <label className="af-check">
                      <input
                        type="checkbox"
                        checked={row.exposed}
                        onChange={(e) => updateInput(row.key, { exposed: e.target.checked })}
                      />
                      Edytowalny na klocku
                    </label>
                    <label className="af-check">
                      <input
                        type="checkbox"
                        checked={row.required}
                        onChange={(e) => updateInput(row.key, { required: e.target.checked })}
                      />
                      Wymagany
                    </label>
                  </div>
                  {row.defaultValue !== undefined && (
                    <small>
                      Wartość domyślna: <code>{JSON.stringify(row.defaultValue)}</code>
                    </small>
                  )}
                </>
              )}
            </div>
          ))}
          <div className="af-section-title">2. Dane zwracane na zewnątrz</div>
          <p className="af-hint">
            Udostępnij całą odpowiedź albo wybrane pole. Wyjścia będą dostępne do podłączania
            kolejnych kroków.
          </p>
          {outputs.map((row) => (
            <div className={`af-builder-row ${row.enabled ? 'is-enabled' : ''}`} key={row.key}>
              <label className="af-check">
                <input
                  type="checkbox"
                  checked={row.enabled}
                  onChange={(e) => updateOutput(row.key, { enabled: e.target.checked })}
                />
                {nodes.find((n) => n.id === row.nodeId)?.name} → {row.port}
              </label>
              {row.enabled && (
                <>
                  <label className="af-field">
                    Źródło wyjścia
                    <select
                      value={`${row.nodeId}:${row.port}`}
                      onChange={(e) => {
                        const [nodeId, port] = e.target.value.split(':');
                        updateOutput(row.key, { nodeId, port, originalBinding: undefined });
                      }}
                    >
                      {row.originalBinding && row.originalBinding.kind !== 'reference' && (
                        <option value=":">Zapisane powiązanie ({row.originalBinding.kind})</option>
                      )}
                      {nodes.flatMap((n) =>
                        outputPorts(n, project).map((p) => (
                          <option key={`${n.id}:${p.name}`} value={`${n.id}:${p.name}`}>
                            {n.name} → {p.label ?? p.name}
                          </option>
                        )),
                      )}
                    </select>
                  </label>
                  <div className="af-inline">
                    <label className="af-field">
                      Nazwa wyjścia
                      <input
                        value={row.name}
                        onChange={(e) => updateOutput(row.key, { name: e.target.value })}
                      />
                    </label>
                    <label className="af-field">
                      Ścieżka (opcjonalnie)
                      <input
                        value={row.path}
                        onChange={(e) => updateOutput(row.key, { path: e.target.value })}
                        placeholder="np. data.id"
                      />
                    </label>
                  </div>
                  {nodes.find((n) => n.id === row.nodeId) && (
                    <PathPicker
                      sample={sampleOutput(
                        nodes.find((n) => n.id === row.nodeId)!,
                        row.port,
                        project,
                      )}
                      value={row.path}
                      onChange={(path) => updateOutput(row.key, { path })}
                    />
                  )}
                  <div className="af-inline">
                    <label className="af-field">
                      Typ
                      <TypeSelect
                        value={row.type}
                        onChange={(type) => updateOutput(row.key, { type })}
                      />
                    </label>
                    <label className="af-field">
                      Kontrakt
                      <select
                        value={row.contractId ?? ''}
                        onChange={(e) => updateOutput(row.key, { contractId: e.target.value })}
                      >
                        <option value="">Bez kontraktu</option>
                        {project.contracts.map((c) => (
                          <option key={c.id} value={c.id}>
                            {c.name}
                          </option>
                        ))}
                      </select>
                    </label>
                  </div>
                </>
              )}
            </div>
          ))}
          <button
            type="button"
            disabled={!nodes.length}
            onClick={() => {
              const n = nodes[nodes.length - 1];
              const p = outputPorts(n, project)[0];
              if (p)
                setOutputs([
                  ...outputs,
                  {
                    key: uid('output'),
                    enabled: true,
                    name: `output_${outputs.length + 1}`,
                    type: p.type,
                    contractId: p.contractId,
                    nodeId: n.id,
                    port: p.name,
                    path: '',
                  },
                ]);
            }}
          >
            + Dodatkowe wyjście
          </button>
          <div className="af-section-title">3. Dostępność w menu „Dodaj akcję”</div>
          <label className="af-field">
            Argument automatycznie zasilany przez poprzedni klocek
            <select
              value={action}
              onChange={(e) => {
                setAction(e.target.value);
                const row = inputs.find((i) => i.key === e.target.value);
                setActionType(row?.type ?? 'any');
                setActionContract(row?.contractId ?? '');
              }}
            >
              <option value="">Tylko biblioteka — bez akcji kontekstowej</option>
              {activeInputs.map((i) => (
                <option value={i.key} key={i.key}>
                  {i.name}
                </option>
              ))}
            </select>
          </label>
          {action && (
            <div className="af-inline">
              <label className="af-field">
                Pokaż dla danych typu
                <TypeSelect value={actionType} onChange={setActionType} />
              </label>
              <label className="af-field">
                Lub kontraktu projektu
                <select value={actionContract} onChange={(e) => setActionContract(e.target.value)}>
                  <option value="">Dopasuj według typu</option>
                  {project.contracts.map((c) => (
                    <option key={c.id} value={c.id}>
                      {c.name}
                    </option>
                  ))}
                </select>
              </label>
            </div>
          )}
          {outsideOrders.length > 0 && (
            <p className="af-hint">
              Połączenia kolejności wychodzące poza zaznaczenie pozostają w scenariuszu. W
              komponencie zapisujemy tylko jego wewnętrzną kolejność.
            </p>
          )}
          {saveError && (
            <div className="af-validation" role="alert">
              {saveError}
            </div>
          )}
          {errors.length > 0 && (
            <div className="af-validation">
              {errors.map((e) => (
                <p key={e}>{e}</p>
              ))}
            </div>
          )}
        </div>
        <footer className="af-modal-footer">
          <span>
            {initialDefinition
              ? 'Nowa rewizja nie zmienia istniejących instancji.'
              : 'Powstanie rewizja 1. Oryginalny graf pozostanie w scenariuszu.'}
          </span>
          <button type="button" onClick={onClose}>
            Anuluj
          </button>
          <button type="button" className="af-primary" disabled={errors.length > 0} onClick={save}>
            Zapisz komponent
          </button>
        </footer>
      </section>
    </div>
  );
}

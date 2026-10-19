import { ResponseBindingPicker } from '../variables/ResponseBindingPicker';
import { reference } from '../../../shared/model';
import { writeVariable } from '../../../shared/variables';
import { VariableTextInput, VariableBindingsContext } from '../variables/VariableTextInput';
import { bindVariableText } from '../../../shared/variables';
import { OperatorSelect } from '../operators/OperatorSelect';
import { changeOperator } from '../../../shared/operators';
import { useState } from 'react';
import { definition, getPath, inputPorts, kindLabels, outputPorts } from '../../../shared/graph';
import type {
  Graph,
  Json,
  NodeConfig,
  Port,
  Predicate,
  Project,
  StepResult,
  TestNode,
} from '../../../shared/model';
import { BindingEditor } from './BindingEditor';
import { JsonEditor, JsonTree, PathPicker, sampleOutput, StringMap, TypeSelect } from './fields';
import './inspector.css';
import { FlowConnections } from '../graph/FlowConnections';

export interface NodeInspectorProps {
  node: TestNode;
  graph: Graph;
  project: Project;
  flowGraph?: Graph;
  onGraphChange?: (graph: Graph) => void;
  onChange: (node: TestNode) => void;
  onEditComponent: (node: TestNode) => void;
  onAssertField: (nodeId: string, port: string, path: string, value: Json) => void;
  onAssignField?: (nodeId: string, port: string, path: string) => void;
  step?: StepResult;
  parameters?: Record<string, Json>;
}
const predicates: Record<Predicate, string> = {
  equals: 'Jest równe',
  notEquals: 'Jest różne od',
  greater: 'Jest większe niż',
  less: 'Jest mniejsze niż',
  exists: 'Istnieje (także null)',
  notEmpty: 'Nie jest puste',
  contains: 'Zawiera',
  type: 'Ma typ',
  approximately: 'Jest równe z tolerancją',
};
export function NodeInspector({
  node,
  graph,
  project,
  onChange,
  flowGraph,
  onGraphChange,
  onEditComponent,
  onAssertField,
  onAssignField,
  step,
  parameters,
}: NodeInspectorProps) {
  const [choosingResponse, setChoosingResponse] = useState(false);
  const [newInput, setNewInput] = useState('');
  const [tab, setTab] = useState<'config' | 'data'>('config');
  const patch = (config: Partial<NodeConfig>) =>
    onChange({ ...node, config: { ...node.config, ...config } });
  const ports = inputPorts(node, project);
  const changePort = (index: number, changes: Partial<Port>) =>
    patch({
      inputPorts: (node.config.inputPorts ?? []).map((p, i) =>
        i === index ? { ...p, ...changes } : p,
      ),
    });
  const addInput = () => {
    const name = newInput.trim();
    if (!name || Object.hasOwn(node.inputs, name) || ports.some((p) => p.name === name)) return;
    onChange({
      ...node,
      inputs: { ...node.inputs, [name]: { kind: 'literal', value: '' } },
      config:
        node.kind === 'request'
          ? {
              ...node.config,
              inputPorts: [
                ...(node.config.inputPorts ?? []),
                { name, type: 'any', required: true, exposed: true },
              ],
            }
          : node.config,
    });
    setNewInput('');
  };
  const removeInput = (name: string) =>
    onChange({
      ...node,
      inputs: Object.fromEntries(Object.entries(node.inputs).filter(([key]) => key !== name)),
      config: {
        ...node.config,
        inputPorts: node.config.inputPorts?.filter((p) => p.name !== name),
      },
    });
  const component = definition(project, node);
  const newest = component
    ? Math.max(...project.components.filter((c) => c.id === component.id).map((c) => c.revision))
    : 0;
  const actualOutputs =
    step?.outputs ??
    Object.fromEntries(
      outputPorts(node, project).flatMap((p) => {
        const sample = sampleOutput(node, p.name, project);
        return sample === undefined ? [] : [[p.name, sample]];
      }),
    );
  const selectedBinding = node.inputs.value;
  const selectSource =
    selectedBinding?.kind === 'reference'
      ? graph.nodes.find((n) => n.id === selectedBinding.nodeId)
      : undefined;
  return (
    <VariableBindingsContext.Provider value={node.inputs}>
      <aside className="af-inspector af-editor" aria-label="Konfiguracja klocka">
        <header className="af-inspector-header">
          <span className="af-eyebrow">{kindLabels[node.kind]}</span>
          <h2>{node.name}</h2>
          <div className="af-tabs">
            <button
              type="button"
              className={tab === 'config' ? 'is-active' : ''}
              onClick={() => setTab('config')}
            >
              Konfiguracja
            </button>
            <button
              type="button"
              className={tab === 'data' ? 'is-active' : ''}
              onClick={() => setTab('data')}
            >
              Dane wyjściowe
            </button>
          </div>
        </header>
        <div className="af-inspector-content">
          {tab === 'config' ? (
            <>
              <label className="af-field">
                Nazwa klocka
                <input
                  value={node.name}
                  onChange={(e) => onChange({ ...node, name: e.target.value })}
                />
              </label>
              {flowGraph && onGraphChange && (
                <FlowConnections
                  graph={flowGraph}
                  project={project}
                  node={node}
                  onChange={onGraphChange}
                />
              )}
              {node.kind === 'trigger' && (
                <p className="af-hint">
                  Wyzwalacz ręczny. Kliknięcie Uruchom rozpoczyna tę ścieżkę. Połącz wyjście Dalej z
                  pierwszym krokiem.
                </p>
              )}
              {node.kind === 'if' && (
                <p className="af-hint">
                  Wynik porównania wybiera wyjście Tak albo Nie. Fałsz nie oznacza błędu testu.
                  Połącz obie gałęzie z odpowiednimi krokami.
                </p>
              )}
              {node.kind === 'variable' && (
                <>
                  <button type="button" onClick={() => setChoosingResponse(true)}>
                    Wybierz wartość z odpowiedzi
                  </button>
                  <p className="af-hint">
                    Wskaż request i pole odpowiedzi. Zmienna pobierze aktualną wartość przy każdym
                    uruchomieniu.
                  </p>
                </>
              )}
              {node.kind === 'request' && (
                <>
                  <div className="af-inline">
                    <label className="af-field af-method">
                      Metoda
                      <select
                        value={node.config.method ?? 'GET'}
                        onChange={(e) => patch({ method: e.target.value })}
                      >
                        {['GET', 'POST', 'PUT', 'PATCH', 'DELETE', 'HEAD', 'OPTIONS'].map((m) => (
                          <option key={m}>{m}</option>
                        ))}
                      </select>
                    </label>
                    <label className="af-field">
                      URL lub ścieżka
                      <VariableTextInput
                        aria-label="URL lub ścieżka"
                        value={node.config.url ?? ''}
                        placeholder="/endpoint/:argument"
                        onValueChange={(url) => patch({ url })}
                        onVariableDrop={(variable, selection) =>
                          onChange(
                            bindVariableText(node, variable, selection, (next, url) => ({
                              ...next,
                              config: { ...next.config, url },
                            })),
                          )
                        }
                      />
                    </label>
                  </div>
                  <p className="af-hint">
                    Adres względny korzysta z base URL środowiska. Argumenty: <code>:argument</code>{' '}
                    lub <code>{'{{argument}}'}</code>.
                  </p>
                  <label className="af-field">
                    Timeout (ms)
                    <input
                      type="number"
                      min="1"
                      max="60000"
                      value={node.config.timeoutMs ?? 10000}
                      onChange={(e) =>
                        patch({ timeoutMs: Math.min(60000, Math.max(1, Number(e.target.value))) })
                      }
                    />
                  </label>
                  <StringMap
                    label="Parametry query"
                    value={node.config.query ?? {}}
                    onChange={(query) => patch({ query })}
                    onVariableDrop={(key, variable, selection) =>
                      onChange(
                        bindVariableText(node, variable, selection, (next, text) => ({
                          ...next,
                          config: { ...next.config, query: { ...next.config.query, [key]: text } },
                        })),
                      )
                    }
                  />
                  <StringMap
                    label="Nagłówki"
                    value={node.config.headers ?? {}}
                    onChange={(headers) => patch({ headers })}
                    onVariableDrop={(key, variable, selection) =>
                      onChange(
                        bindVariableText(node, variable, selection, (next, text) => ({
                          ...next,
                          config: {
                            ...next.config,
                            headers: { ...next.config.headers, [key]: text },
                          },
                        })),
                      )
                    }
                  />
                  <p className="af-hint">
                    Wartości mogą używać {'{{argumentu}}'}. Token ustaw jako argument ze źródłem
                    „Środowisko / sekret”, np. nagłówek Authorization = Bearer {'{{token}}'}.
                  </p>
                </>
              )}
              {node.kind === 'select' && (
                <>
                  <label className="af-field">
                    Ścieżka do pola
                    <VariableTextInput
                      aria-label="Ścieżka do pola"
                      placeholder="np. items[0].name; puste = całość"
                      value={node.config.path ?? ''}
                      onValueChange={(path) => patch({ path })}
                      onVariableDrop={(variable, selection) =>
                        onChange(
                          bindVariableText(node, variable, selection, (next, path) => ({
                            ...next,
                            config: { ...next.config, path },
                          })),
                        )
                      }
                    />
                  </label>
                  {selectSource && selectedBinding?.kind === 'reference' && (
                    <PathPicker
                      sample={
                        getPath(
                          sampleOutput(selectSource, selectedBinding.port, project),
                          selectedBinding.path,
                        ) as Json | undefined
                      }
                      value={node.config.path ?? ''}
                      onChange={(path) => patch({ path })}
                    />
                  )}
                </>
              )}
              {node.kind === 'build' && (
                <label className="af-field">
                  Utwórz
                  <select
                    value={node.config.buildType ?? 'object'}
                    onChange={(e) => patch({ buildType: e.target.value as 'object' | 'array' })}
                  >
                    <option value="object">Obiekt z nazwanych pól</option>
                    <option value="array">Listę z kolejnych wartości</option>
                  </select>
                </label>
              )}
              {node.kind === 'formula' && (
                <label className="af-field">
                  Operacja
                  <OperatorSelect
                    value={node.config.operation ?? 'multiply'}
                    onChange={(operation) => {
                      const next = changeOperator(
                        {
                          kind: 'expression',
                          operator: node.config.operation ?? 'multiply',
                          inputs: node.inputs,
                        },
                        operation,
                      );
                      onChange({
                        ...node,
                        config: { ...node.config, operation },
                        inputs: next.inputs,
                      });
                    }}
                  />
                </label>
              )}
              {(node.kind === 'assert' || node.kind === 'condition' || node.kind === 'if') && (
                <>
                  <label className="af-field">
                    Sprawdzenie
                    <select
                      aria-label="Sprawdzenie"
                      value={node.config.predicate ?? 'equals'}
                      onChange={(e) => {
                        const predicate = e.target.value as Predicate;
                        onChange({
                          ...node,
                          config: { ...node.config, predicate },
                          inputs:
                            predicate === 'type' && node.inputs.expected?.kind === 'literal'
                              ? { ...node.inputs, expected: { kind: 'literal', value: 'string' } }
                              : node.inputs,
                        });
                      }}
                    >
                      {Object.entries(predicates).map(([v, label]) => (
                        <option value={v} key={v}>
                          {label}
                        </option>
                      ))}
                    </select>
                  </label>
                  {node.config.predicate === 'approximately' && (
                    <label className="af-field">
                      Dopuszczalna różnica
                      <input
                        type="number"
                        min="0"
                        step="any"
                        value={node.config.tolerance ?? 0.01}
                        onChange={(e) => patch({ tolerance: Math.max(0, Number(e.target.value)) })}
                      />
                    </label>
                  )}
                  {node.kind === 'condition' && (
                    <p className="af-hint">
                      Niespełniony warunek oznacza niewłaściwe dane i blokuje zależne kroki.
                    </p>
                  )}
                </>
              )}
              {node.kind === 'component' && (
                <div className="af-component-info">
                  <p>{component?.description ?? 'Brak definicji tej rewizji.'}</p>
                  <span className="af-badge">Rewizja {node.config.revision}</span>
                  <button type="button" onClick={() => onEditComponent(node)} disabled={!component}>
                    Otwórz wnętrze komponentu ↗
                  </button>
                  {component && newest > component.revision && (
                    <button type="button" onClick={() => patch({ revision: newest })}>
                      Aktualizuj tę instancję do rewizji {newest}
                    </button>
                  )}
                </div>
              )}
              <div className="af-section-title">
                Argumenty <span>{ports.length}</span>
              </div>
              {ports.map((p) => (
                <div className="af-port" key={p.name}>
                  <BindingEditor
                    value={node.inputs[p.name]}
                    port={p}
                    node={node}
                    graph={graph}
                    project={project}
                    parameters={parameters}
                    onChange={(binding) => {
                      const inputs = { ...node.inputs };
                      if (binding) inputs[p.name] = binding;
                      else delete inputs[p.name];
                      onChange({ ...node, inputs });
                    }}
                  />
                  {node.kind === 'request' && p.name !== 'body' && (
                    <details className="af-port-options">
                      <summary>Ustawienia argumentu</summary>
                      <TypeSelect
                        value={p.type}
                        onChange={(type) =>
                          changePort(
                            (node.config.inputPorts ?? []).findIndex((q) => q.name === p.name),
                            { type },
                          )
                        }
                      />
                      <label className="af-check">
                        <input
                          type="checkbox"
                          checked={p.required ?? false}
                          onChange={(e) =>
                            changePort(
                              (node.config.inputPorts ?? []).findIndex((q) => q.name === p.name),
                              { required: e.target.checked },
                            )
                          }
                        />
                        Wymagany
                      </label>
                      <label className="af-check">
                        <input
                          type="checkbox"
                          checked={p.exposed ?? false}
                          onChange={(e) =>
                            changePort(
                              (node.config.inputPorts ?? []).findIndex((q) => q.name === p.name),
                              { exposed: e.target.checked },
                            )
                          }
                        />
                        Pokaż na klocku
                      </label>
                      <button
                        type="button"
                        className="af-danger"
                        onClick={() => removeInput(p.name)}
                      >
                        Usuń argument
                      </button>
                    </details>
                  )}
                  {node.kind === 'build' && (
                    <button type="button" className="af-danger" onClick={() => removeInput(p.name)}>
                      Usuń pole
                    </button>
                  )}
                </div>
              ))}
              {['request', 'build', 'condition'].includes(node.kind) && (
                <div className="af-inline">
                  <input
                    aria-label="Nazwa nowego argumentu"
                    value={newInput}
                    placeholder={node.kind === 'build' ? 'Nazwa pola' : 'Nazwa argumentu'}
                    onChange={(e) => setNewInput(e.target.value)}
                    onKeyDown={(e) => {
                      if (e.key === 'Enter') addInput();
                    }}
                  />
                  <button
                    type="button"
                    disabled={!newInput.trim() || ports.some((p) => p.name === newInput.trim())}
                    onClick={addInput}
                  >
                    Dodaj
                  </button>
                </div>
              )}
              {node.kind === 'condition' && (
                <>
                  <div className="af-section-title">Dodatkowe warunki</div>
                  <label className="af-field">
                    Sposób łączenia
                    <select
                      value={node.config.match ?? 'all'}
                      onChange={(e) => patch({ match: e.target.value as 'all' | 'any' })}
                    >
                      <option value="all">Wszystkie warunki</option>
                      <option value="any">Dowolny warunek</option>
                    </select>
                  </label>
                  {(node.config.predicates ?? []).map((p, i) => (
                    <div className="af-condition-row" key={i}>
                      <select
                        aria-label="Argument warunku"
                        value={p.input}
                        onChange={(e) =>
                          patch({
                            predicates: node.config.predicates!.map((v, j) =>
                              i === j ? { ...v, input: e.target.value } : v,
                            ),
                          })
                        }
                      >
                        {ports.map((p) => (
                          <option key={p.name}>{p.name}</option>
                        ))}
                      </select>
                      <VariableTextInput
                        aria-label="Ścieżka warunku"
                        value={p.path ?? ''}
                        placeholder="Ścieżka (opcjonalnie)"
                        onValueChange={(path) =>
                          patch({
                            predicates: node.config.predicates!.map((value, index) =>
                              index === i ? { ...value, path } : value,
                            ),
                          })
                        }
                        onVariableDrop={(variable, selection) =>
                          onChange(
                            bindVariableText(node, variable, selection, (next, path) => ({
                              ...next,
                              config: {
                                ...next.config,
                                predicates: next.config.predicates!.map((value, index) =>
                                  index === i ? { ...value, path } : value,
                                ),
                              },
                            })),
                          )
                        }
                      />
                      <select
                        aria-label="Porównanie warunku"
                        value={p.predicate}
                        onChange={(e) =>
                          patch({
                            predicates: node.config.predicates!.map((v, j) =>
                              i === j ? { ...v, predicate: e.target.value as Predicate } : v,
                            ),
                          })
                        }
                      >
                        {Object.entries(predicates).map(([v, label]) => (
                          <option value={v} key={v}>
                            {label}
                          </option>
                        ))}
                      </select>
                      <select
                        aria-label="Oczekiwany argument"
                        value={p.expected}
                        onChange={(e) =>
                          patch({
                            predicates: node.config.predicates!.map((v, j) =>
                              i === j ? { ...v, expected: e.target.value } : v,
                            ),
                          })
                        }
                      >
                        {ports.map((p) => (
                          <option key={p.name}>{p.name}</option>
                        ))}
                      </select>
                      <button
                        type="button"
                        className="af-danger"
                        onClick={() =>
                          patch({ predicates: node.config.predicates!.filter((_, j) => j !== i) })
                        }
                      >
                        Usuń warunek
                      </button>
                    </div>
                  ))}
                  <button
                    type="button"
                    onClick={() =>
                      patch({
                        predicates: [
                          ...(node.config.predicates ?? []),
                          {
                            input: 'actual',
                            expected: 'expected',
                            predicate: node.config.predicate ?? 'equals',
                          },
                        ],
                      })
                    }
                  >
                    + Warunek
                  </button>
                  <p className="af-hint">
                    Po dodaniu listy silnik sprawdza warunki z listy zamiast pojedynczego
                    sprawdzenia powyżej.
                  </p>
                </>
              )}
              {node.kind === 'request' && (
                <details className="af-sample">
                  <summary>Kontrakt i przykładowa odpowiedź</summary>
                  <label className="af-field">
                    Kontrakt danych
                    <select
                      value={node.config.contractId ?? ''}
                      onChange={(e) => patch({ contractId: e.target.value || undefined })}
                    >
                      <option value="">Bez nazwanego kontraktu</option>
                      {project.contracts.map((c) => (
                        <option value={c.id} key={c.id}>
                          {c.name}
                        </option>
                      ))}
                    </select>
                  </label>
                  <label className="af-field">
                    Przykładowy status HTTP
                    <input
                      type="number"
                      min="100"
                      max="599"
                      value={node.config.sampleStatus ?? 200}
                      onChange={(e) => patch({ sampleStatus: Number(e.target.value) })}
                    />
                  </label>
                  <JsonEditor
                    label="Przykładowe body"
                    value={node.config.sampleResponse ?? null}
                    onChange={(sampleResponse) => patch({ sampleResponse })}
                  />
                  <p className="af-hint">
                    Przykład służy podpowiedziom i symulacji. Obecność pola w przykładzie nie tworzy
                    asercji.
                  </p>
                </details>
              )}
            </>
          ) : (
            <>
              <p className="af-hint">
                {step
                  ? 'Dane z ostatniego wykonania. Edycja klocka nie aktualizuje tej odpowiedzi.'
                  : 'Przykładowe dane — nie pochodzą z wywołania API.'}
              </p>
              {Object.keys(actualOutputs).length ? (
                Object.entries(actualOutputs).map(([port, value]) => (
                  <section className="af-output" key={port}>
                    <div className="af-section-title">
                      {outputPorts(node, project).find((p) => p.name === port)?.label ?? port}
                    </div>
                    <JsonTree
                      value={value}
                      onPick={(path, value) => onAssertField(node.id, port, path, value)}
                      onAssign={
                        onAssignField ? (path) => onAssignField(node.id, port, path) : undefined
                      }
                      onDragField={(path, data) =>
                        writeVariable(data, {
                          label: (path || port).slice(0, 200),
                          binding: reference(node.id, port, path),
                        })
                      }
                    />
                  </section>
                ))
              ) : (
                <div className="af-empty">
                  Uruchom krok, aby zobaczyć dane. W requeście możesz też przygotować przykładową
                  odpowiedź.
                </div>
              )}
            </>
          )}
        </div>
        {choosingResponse && (
          <ResponseBindingPicker
            key={node.id}
            node={node}
            graph={graph}
            project={project}
            onClose={() => setChoosingResponse(false)}
            onChange={(binding) =>
              onChange({ ...node, inputs: { ...node.inputs, value: binding } })
            }
          />
        )}
      </aside>
    </VariableBindingsContext.Provider>
  );
}

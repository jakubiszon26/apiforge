import { VariableChip } from '../variables/VariableChip';
import { readVariable, VARIABLE_MIME } from '../../../shared/variables';
import { ResponseFieldPicker } from './ResponseFieldPicker';
import { useResponseSource } from './responseData';
import { expression, isOperator, operators } from '../../../shared/operators';
import { references } from '../../../shared/bindings';
import { ExpressionBlock } from '../operators/ExpressionBlock';
import { OPERATOR_MIME } from '../operators/OperatorPalette';
import { useState } from 'react';
import type { Binding, Graph, Json, Port, Project, TestNode } from '../../../shared/model';
import { bindInput, compatible, hasCycle, outputPorts } from '../../../shared/graph';
import { AssertionTypeSelect, JsonEditor, TypeSelect, valueType } from './fields';

interface Props {
  value?: Binding;
  onChange: (binding: Binding | undefined) => void;
  graph: Graph;
  node: TestNode;
  project: Project;
  port: Port;
  parameters?: Record<string, Json>;
  depth?: number;
}
export function BindingEditor({
  value,
  onChange,
  graph,
  node,
  project,
  port,
  parameters = {},
  depth = 0,
}: Props) {
  const [error, setError] = useState('');
  const [dropping, setDropping] = useState(false);
  const current = value ?? { kind: 'literal' as const, value: '' };
  const sources = graph.nodes.filter(
    (n) =>
      n.id !== node.id &&
      outputPorts(n, project).length > 0 &&
      !hasCycle(
        bindInput(graph, node.id, port.name, {
          kind: 'reference',
          nodeId: n.id,
          port: outputPorts(n, project)[0]?.name ?? 'value',
          path: '',
        }),
      ),
  );
  const update = (binding: Binding) => {
    if (references(binding).length && hasCycle(bindInput(graph, node.id, port.name, binding))) {
      setError('To połączenie utworzyłoby cykl. Wybierz inny krok.');
      return;
    }
    setError('');
    onChange(binding);
  };
  const literalType = current.kind === 'literal' ? valueType(current.value) : 'any';
  const source =
    current.kind === 'reference' ? graph.nodes.find((n) => n.id === current.nodeId) : undefined;
  const boundVariable =
    current.kind === 'parameter' || current.kind === 'environment'
      ? { label: current.name, binding: current }
      : current.kind === 'reference' && source?.kind === 'variable' && !current.path
        ? { label: source.name, binding: current }
        : undefined;
  const outputs = source ? outputPorts(source, project) : [];
  const { sample: sourceSample, origin: fieldOrigin } = useResponseSource(
    source,
    current.kind === 'reference' ? current.port : '',
    project,
  );
  return (
    <div
      className={`af-binding af-binding-drop-target ${dropping ? 'is-drop-target' : ''}`}
      onDropCapture={(event) => {
        const target = event.target;
        if (
          !(target instanceof HTMLElement) ||
          target.closest('.af-binding-drop-target') !== event.currentTarget
        )
          return;
        const variable = readVariable(event.dataTransfer);
        if (!variable) return;
        event.preventDefault();
        event.stopPropagation();
        setDropping(false);
        if (
          current.kind === 'reference' &&
          target.getAttribute('aria-label') === `Ścieżka ${port.name}`
        ) {
          update({
            kind: 'expression',
            operator: 'field',
            inputs: { left: { ...current, path: '' }, right: variable.binding },
          });
        } else update(variable.binding);
      }}
      onDragOver={(event) => {
        if (event.dataTransfer.types.includes(VARIABLE_MIME)) {
          event.preventDefault();
          event.stopPropagation();
          setDropping(true);
          return;
        }
        if (depth < 11 && event.dataTransfer.types.includes(OPERATOR_MIME)) {
          event.preventDefault();
          event.stopPropagation();
          setDropping(true);
        }
      }}
      onDragLeave={() => setDropping(false)}
      onDrop={(event) => {
        const name = event.dataTransfer.getData(OPERATOR_MIME);
        if (!isOperator(name) || depth >= 11) return;
        event.preventDefault();
        event.stopPropagation();
        setDropping(false);
        const block = expression(name);
        if (value && operators[name].inputs.some((port) => port.name === 'left'))
          block.inputs.left = value;
        update(block);
      }}
    >
      <div className="af-binding-head">
        <span>
          {port.label ?? port.name}
          {port.required && <b title="Wymagane"> *</b>}
        </span>
        <select
          aria-label={`Źródło ${port.name}`}
          value={value?.kind ?? 'unset'}
          onChange={(e) => {
            const kind = e.target.value;
            if (kind === 'unset') {
              onChange(undefined);
              return;
            }
            if (kind === 'expression') {
              const block = expression('add');
              if (value) block.inputs.left = value;
              update(block);
            }
            if (kind === 'literal')
              update({
                kind,
                value:
                  port.defaultValue ??
                  (port.type === 'number'
                    ? 0
                    : port.type === 'boolean'
                      ? false
                      : port.type === 'object'
                        ? {}
                        : port.type === 'array'
                          ? []
                          : ''),
              });
            if (kind === 'reference') {
              const n = sources[0];
              if (n)
                update({
                  kind,
                  nodeId: n.id,
                  port: outputPorts(n, project)[0]?.name ?? 'value',
                  path: '',
                });
            }
            if (kind === 'parameter')
              update({ kind, name: Object.keys(parameters)[0] ?? port.name });
            if (kind === 'environment')
              update({ kind, name: Object.keys(project.environment.variables)[0] ?? 'baseUrl' });
          }}
        >
          <option value="unset">Nie ustawiono</option>
          <option value="literal">Wpisana wartość</option>
          <option value="reference" disabled={!sources.length}>
            Wynik innego kroku
          </option>
          <option value="expression" disabled={depth >= 11}>
            Operator · zagnieżdżony blok
          </option>
          <option value="parameter">Parametr</option>
          <option value="environment">Środowisko / sekret</option>
        </select>
      </div>
      {boundVariable && (
        <VariableChip variable={boundVariable} onRemove={() => onChange(undefined)} />
      )}
      {value?.kind === 'expression' && (
        <ExpressionBlock
          value={value}
          onUnwrap={() => {
            const left = value.inputs.left;
            if (left) update(left);
            else onChange(undefined);
          }}
          onChange={update}
          node={node}
          graph={graph}
          project={project}
          parameters={parameters}
          depth={depth}
        />
      )}
      {value?.kind === 'expression' &&
        port.type !== 'any' &&
        operators[value.operator].output !== 'any' &&
        operators[value.operator].output !== port.type && (
          <span className="af-error">
            Ten operator zwraca {operators[value.operator].output}, a argument wymaga {port.type}.
          </span>
        )}
      {value?.kind === 'literal' && node.config.predicate === 'type' && port.name === 'expected' ? (
        <AssertionTypeSelect
          value={String(value.value)}
          onChange={(type) => update({ kind: 'literal', value: type })}
        />
      ) : (
        value?.kind === 'literal' && (
          <>
            {(port.type === 'any' || literalType !== port.type) && (
              <TypeSelect
                value={literalType}
                onChange={(type) =>
                  update({
                    kind: 'literal',
                    value:
                      type === 'number'
                        ? 0
                        : type === 'boolean'
                          ? false
                          : type === 'object'
                            ? {}
                            : type === 'array'
                              ? []
                              : type === 'any'
                                ? null
                                : '',
                  })
                }
              />
            )}
            {literalType === 'string' ? (
              <input
                aria-label={`Wartość ${port.name}`}
                value={String(value.value)}
                onChange={(e) => update({ kind: 'literal', value: e.target.value })}
              />
            ) : literalType === 'number' ? (
              <input
                type="number"
                aria-label={`Wartość ${port.name}`}
                value={Number(value.value)}
                onChange={(e) => {
                  if (e.target.value !== '' && Number.isFinite(Number(e.target.value)))
                    update({ kind: 'literal', value: Number(e.target.value) });
                }}
              />
            ) : literalType === 'boolean' ? (
              <select
                aria-label={`Wartość ${port.name}`}
                value={String(value.value)}
                onChange={(e) => update({ kind: 'literal', value: e.target.value === 'true' })}
              >
                <option value="true">Tak (true)</option>
                <option value="false">Nie (false)</option>
              </select>
            ) : literalType === 'any' ? (
              <small>Jawna wartość null</small>
            ) : (
              <JsonEditor
                value={value.value}
                onChange={(v) => update({ kind: 'literal', value: v })}
                label="Obiekt lub lista"
              />
            )}
          </>
        )
      )}
      {current.kind === 'reference' && (
        <details
          className={`af-reference-fields ${depth === 0 && !boundVariable ? 'is-root' : ''}`}
          open={depth === 0 && !boundVariable ? true : undefined}
        >
          <summary>
            {source?.name ?? 'Źródło'} → {current.port}
            {current.path ? `.${current.path}` : ''}
          </summary>
          <select
            aria-label={`Krok źródłowy ${port.name}`}
            value={current.nodeId}
            onChange={(e) => {
              const n = sources.find((n) => n.id === e.target.value)!;
              update({
                ...current,
                nodeId: n.id,
                port: outputPorts(n, project)[0]?.name ?? 'value',
                path: '',
              });
            }}
          >
            {!sources.some((n) => n.id === current.nodeId) && (
              <option value={current.nodeId}>Brak dostępnego źródła</option>
            )}
            {sources.map((n) => (
              <option value={n.id} key={n.id}>
                {n.name}
              </option>
            ))}
          </select>
          <select
            aria-label={`Wyjście ${port.name}`}
            value={current.port}
            onChange={(e) => update({ ...current, port: e.target.value })}
          >
            {outputs.map((p) => (
              <option value={p.name} key={p.name}>
                {p.label ?? p.name}
              </option>
            ))}
          </select>
          <ResponseFieldPicker
            sample={sourceSample}
            value={current.path}
            port={current.port}
            origin={fieldOrigin}
            onChange={(path) => update({ ...current, path })}
          />
          <label className="af-field">
            Własna ścieżka (opcjonalnie)
            <input
              aria-label={`Ścieżka ${port.name}`}
              placeholder="np. data.items[0].name; puste = całość"
              value={current.path}
              onChange={(event) => update({ ...current, path: event.target.value })}
            />
          </label>
          {!current.path &&
            outputs.find((p) => p.name === current.port) &&
            !compatible(
              outputs.find((p) => p.name === current.port)!,
              port,
            ) && (
              <span className="af-error">
                Typ wyjścia nie pasuje do wejścia. Wybierz odpowiednie pole lub inne źródło.
              </span>
            )}
        </details>
      )}
      {(current.kind === 'parameter' || current.kind === 'environment') && (
        <>
          <input
            aria-label={`Nazwa ${port.name}`}
            list={`names-${node.id}-${port.name}`}
            value={current.name}
            onChange={(e) => update({ ...current, name: e.target.value })}
          />
          <datalist id={`names-${node.id}-${port.name}`}>
            {(current.kind === 'parameter'
              ? Object.keys(parameters)
              : [
                  'baseUrl',
                  ...Object.keys(project.environment.variables),
                  ...project.environment.secretNames,
                ]
            ).map((name) => (
              <option value={name} key={name} />
            ))}
          </datalist>
          <small>
            {current.kind === 'environment'
              ? 'Sekrety są dostępne tylko w bieżącej sesji.'
              : 'Parametr scenariusza lub argument komponentu.'}
          </small>
        </>
      )}
      {!value && port.required && <span className="af-error">Ustaw wymagany argument.</span>}
      {error && <span className="af-error">{error}</span>}
    </div>
  );
}

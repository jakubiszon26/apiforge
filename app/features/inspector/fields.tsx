import { VariableTextInput } from '../variables/VariableTextInput';
import type { VariableBlock, TextSelection } from '../../../shared/variables';
import { useEffect, useState } from 'react';
import type { Json, Project, TestNode, ValueType } from '../../../shared/model';
import { definition, getPath } from '../../../shared/graph';
import { childPath, samplePaths, supportedKey } from './jsonPaths';
export { samplePaths } from './jsonPaths';

export const typeNames: Record<ValueType, string> = {
  any: 'Dowolne dane',
  string: 'Tekst',
  number: 'Liczba',
  boolean: 'Tak / nie',
  object: 'Obiekt',
  array: 'Lista',
};
export function valueType(value: Json): ValueType {
  return value === null ? 'any' : Array.isArray(value) ? 'array' : (typeof value as ValueType);
}
export function TypeSelect({
  value,
  onChange,
}: {
  value: ValueType;
  onChange: (value: ValueType) => void;
}) {
  return (
    <select
      aria-label="Typ danych"
      value={value}
      onChange={(e) => onChange(e.target.value as ValueType)}
    >
      {Object.entries(typeNames).map(([v, label]) => (
        <option key={v} value={v}>
          {label}
        </option>
      ))}
    </select>
  );
}
export function JsonEditor({
  value,
  onChange,
  label = 'Dane JSON',
}: {
  value: Json;
  onChange: (value: Json) => void;
  label?: string;
}) {
  const serialized = JSON.stringify(value, null, 2);
  const [draft, setDraft] = useState(serialized);
  const [error, setError] = useState('');
  useEffect(() => {
    setDraft(serialized);
    setError('');
  }, [serialized]);
  return (
    <label className="af-field">
      {label}
      <textarea
        className="af-code"
        rows={6}
        value={draft}
        onChange={(e) => {
          setDraft(e.target.value);
          try {
            const data = JSON.parse(e.target.value) as Json;
            setError('');
            onChange(data);
          } catch {
            setError('Nieprawidłowy JSON — ostatnia poprawna wartość pozostaje zapisana.');
          }
        }}
      />
      {error && <span className="af-error">{error}</span>}
    </label>
  );
}
export function StringMap({
  value,
  onChange,
  label,
  onVariableDrop,
}: {
  value: Record<string, string>;
  onChange: (value: Record<string, string>) => void;
  onVariableDrop?: (key: string, variable: VariableBlock, selection: TextSelection) => void;
  label: string;
}) {
  const [name, setName] = useState('');
  return (
    <div className="af-field">
      <span>{label}</span>
      {Object.entries(value).map(([key, v]) => (
        <div className="af-map-row" key={key}>
          <code title={key}>{key}</code>
          <VariableTextInput
            aria-label={`${label}: ${key}`}
            value={v}
            onValueChange={(text) => onChange({ ...value, [key]: text })}
            onVariableDrop={
              onVariableDrop
                ? (variable, selection) => onVariableDrop(key, variable, selection)
                : undefined
            }
          />
          <button
            type="button"
            className="af-icon-button"
            aria-label={`Usuń ${key}`}
            onClick={() =>
              onChange(Object.fromEntries(Object.entries(value).filter(([k]) => k !== key)))
            }
          >
            ×
          </button>
        </div>
      ))}
      <div className="af-inline">
        <input
          aria-label={`Nowa nazwa: ${label}`}
          placeholder="Nazwa"
          value={name}
          onChange={(e) => setName(e.target.value)}
        />
        <button
          type="button"
          disabled={!name.trim() || Object.hasOwn(value, name.trim())}
          onClick={() => {
            onChange({ ...value, [name.trim()]: '' });
            setName('');
          }}
        >
          Dodaj
        </button>
      </div>
    </div>
  );
}

export function JsonTree({
  value,
  onPick,
  onAssign,
  onDragField,
  path = '',
  depth = 0,
  supported = true,
}: {
  value: Json;
  onPick?: (path: string, value: Json) => void;
  onAssign?: (path: string) => void;
  onDragField?: (path: string, data: DataTransfer) => void;
  path?: string;
  depth?: number;
  supported?: boolean;
}) {
  const compound = value !== null && typeof value === 'object';
  const entries = compound ? Object.entries(value) : [];
  const text = compound
    ? `${Array.isArray(value) ? 'Lista' : 'Obiekt'} · ${entries.length}`
    : JSON.stringify(value);
  return (
    <div className="af-json-tree">
      <div className="af-json-row">
        <code
          title={path}
          className={onDragField && supported ? 'af-draggable-response-field' : undefined}
          draggable={Boolean(onDragField) && supported}
          onDragStart={(event) => onDragField?.(path, event.dataTransfer)}
        >
          {path || '$'}
        </code>
        <span title={text}>{text}</span>
        {onPick && (
          <button
            type="button"
            disabled={!supported}
            title={
              supported
                ? `Sprawdź ${path || 'całą wartość'}`
                : 'To zastrzeżona nazwa pola. Porównaj obiekt nadrzędny.'
            }
            onClick={() => onPick(path, value)}
          >
            {supported ? 'Sprawdź' : 'Niedostępne'}
          </button>
        )}
        {onAssign && (
          <button
            type="button"
            disabled={!supported}
            title={`Przypisz ${path || 'całą wartość'} do zmiennej`}
            onClick={() => onAssign(path)}
          >
            Do zmiennej
          </button>
        )}
      </div>
      {depth < 10 &&
        entries
          .slice(0, 100)
          .map(([key, v]) => (
            <JsonTree
              key={key}
              value={v}
              path={childPath(path, key, Array.isArray(value))}
              depth={depth + 1}
              supported={supported && (Array.isArray(value) || supportedKey(key))}
              onPick={onPick}
              onAssign={onAssign}
              onDragField={onDragField}
            />
          ))}
      {entries.length > 100 && <small>Wyświetlono pierwsze 100 pól.</small>}
    </div>
  );
}

export function sampleOutput(
  node: TestNode,
  port: string,
  project: Project,
  depth = 0,
): Json | undefined {
  if (depth > 10) return undefined;
  if (node.kind === 'request') {
    if (port === 'body')
      return (
        node.config.sampleResponse ??
        project.contracts.find((c) => c.id === node.config.contractId)?.sample
      );
    if (port === 'status') return node.config.sampleStatus ?? 200;
    return undefined;
  }
  if (node.kind === 'component') {
    const component = definition(project, node);
    const binding = component?.outputs.find((p) => p.name === port)?.binding;
    if (binding?.kind === 'literal') return binding.value;
    if (binding?.kind === 'reference') {
      const source = component?.graph.nodes.find((n) => n.id === binding.nodeId);
      return source
        ? (getPath(sampleOutput(source, binding.port, project, depth + 1), binding.path) as
            Json | undefined)
        : undefined;
    }
  }
  return undefined;
}
export function PathPicker({
  sample,
  value,
  onChange,
}: {
  sample: Json | undefined;
  value: string;
  onChange: (path: string) => void;
}) {
  const paths = samplePaths(sample);
  if (!paths.length) return null;
  return (
    <label className="af-path-picker">
      Wybierz pole z przykładu
      <select
        aria-label="Pole z przykładowej odpowiedzi"
        value={paths.includes(value) || !value ? value : '__custom__'}
        onChange={(e) => onChange(e.target.value)}
      >
        <option value="">Cała wartość</option>
        {value && !paths.includes(value) && <option value="__custom__">Własna ścieżka</option>}
        {paths.map((p) => (
          <option key={p} value={p}>
            {p}
          </option>
        ))}
      </select>
      <small>Struktura z przykładu — podpowiedź, nie gwarancja.</small>
    </label>
  );
}

// Assertion types describe actual JSON values; “any” would not test a type.
export function AssertionTypeSelect({
  value,
  onChange,
  className,
}: {
  value: string;
  onChange: (value: string) => void;
  className?: string;
}) {
  const types = {
    string: 'Tekst',
    number: 'Liczba',
    boolean: 'Tak / nie',
    object: 'Obiekt',
    array: 'Lista',
    null: 'Null',
  };
  return (
    <select
      className={className}
      aria-label="Oczekiwany typ"
      value={value}
      onChange={(event) => onChange(event.target.value)}
    >
      {!Object.hasOwn(types, value) && <option value="">Wybierz typ</option>}
      {Object.entries(types).map(([type, label]) => (
        <option key={type} value={type}>
          {label}
        </option>
      ))}
    </select>
  );
}

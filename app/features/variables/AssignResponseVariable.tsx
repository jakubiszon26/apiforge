import { useState } from 'react';
import type { Graph, TestNode } from '../../../shared/model';
import { useModalFocus } from '../inspector/useModalFocus';

export function AssignResponseVariable({
  graph,
  source,
  port,
  path,
  onSave,
  onClose,
}: {
  graph: Graph;
  source: TestNode;
  port: string;
  path: string;
  onSave: (name: string, targetId?: string) => string | undefined;
  onClose: () => void;
}) {
  const variables = graph.nodes.filter((node) => node.kind === 'variable');
  const [target, setTarget] = useState('');
  const suggested = path.match(/(?:^|\.)([^.[\]]+)$/)?.[1] || 'Wartość odpowiedzi';
  const [name, setName] = useState(suggested);
  const [error, setError] = useState('');
  const dialog = useModalFocus<HTMLFormElement>(onClose);
  return (
    <div
      className="af-modal-backdrop"
      onClick={(event) => {
        if (event.target === event.currentTarget) onClose();
      }}
    >
      <form
        ref={dialog}
        tabIndex={-1}
        className="af-modal af-editor af-response-variable-dialog"
        role="dialog"
        aria-modal="true"
        aria-label="Przypisz pole do zmiennej"
        onSubmit={(event) => {
          event.preventDefault();
          const error = onSave(name, target || undefined);
          if (error) setError(error);
        }}
      >
        <header className="af-modal-header">
          <div>
            <h2>Przypisz pole do zmiennej</h2>
            <p>
              {source.name} → {port}
              {path ? `.${path}` : ''}
            </p>
          </div>
          <button type="button" aria-label="Zamknij przypisanie" onClick={onClose}>
            ×
          </button>
        </header>
        <div className="af-modal-body">
          <label className="af-field">
            Zmienna
            <select
              aria-label="Zmienna docelowa"
              value={target}
              onChange={(event) => {
                setTarget(event.target.value);
                setName(
                  variables.find((node) => node.id === event.target.value)?.name ?? suggested,
                );
                setError('');
              }}
            >
              <option value="">Utwórz nową zmienną</option>
              {variables.map((node) => (
                <option key={node.id} value={node.id}>
                  {node.name}
                </option>
              ))}
            </select>
          </label>
          <label className="af-field">
            Nazwa zmiennej
            <input data-autofocus value={name} onChange={(event) => setName(event.target.value)} />
          </label>
          <p className="af-hint">
            Zmienna pobierze to pole z odpowiedzi przy każdym wykonaniu. Zachowa jego typ. Potem
            przeciągnij jej bloczek do porównania lub innego argumentu.
          </p>
          {error && (
            <p className="af-error" role="alert">
              {error}
            </p>
          )}
        </div>
        <footer className="af-modal-footer">
          <button type="button" onClick={onClose}>
            Anuluj
          </button>
          <button type="submit" className="af-primary" disabled={!name.trim()}>
            Przypisz zmienną
          </button>
        </footer>
      </form>
    </div>
  );
}

import { useMemo, useState, type ReactNode } from 'react';
import { createPortal } from 'react-dom';
import type { Json } from '../../../shared/model';
import { getPath } from '../../../shared/graph';
import { collectResponseFields } from './responseFields';
import { useModalFocus } from './useModalFocus';
import './response-fields.css';

const types: Record<string, string> = {
  string: 'Tekst',
  number: 'Liczba',
  boolean: 'Tak / nie',
  object: 'Obiekt',
  array: 'Lista',
  null: 'Null',
};
interface Props {
  sample: Json | undefined;
  value: string;
  onChange: (path: string) => void;
  origin: string;
  port: string;
  sourceControl?: ReactNode;
}
export function ResponseFieldDialog({
  sample,
  value,
  onChange,
  origin,
  port,
  onClose,
  sourceControl,
}: Props & { onClose: () => void }) {
  const [search, setSearch] = useState('');
  const fields = useMemo(() => collectResponseFields(sample), [sample]);
  const visible = fields.filter((field) =>
    `${field.path} ${field.preview} ${types[field.type]}`
      .toLocaleLowerCase('pl')
      .includes(search.trim().toLocaleLowerCase('pl')),
  );
  const dialog = useModalFocus<HTMLDivElement>(onClose);
  return createPortal(
    <div
      className="af-modal-backdrop"
      onClick={(event) => {
        if (event.target === event.currentTarget) onClose();
      }}
    >
      <div
        ref={dialog}
        tabIndex={-1}
        role="dialog"
        aria-modal="true"
        aria-label="Wybierz pole odpowiedzi"
        className="af-modal af-editor af-response-field-dialog"
      >
        <header className="af-modal-header">
          <div>
            <h2>Wybierz pole odpowiedzi</h2>
            <p>{origin}</p>
          </div>
          <button type="button" onClick={onClose} aria-label="Zamknij wybór pola">
            ×
          </button>
        </header>
        {sourceControl && <div className="af-response-source-control">{sourceControl}</div>}
        <div className="af-response-field-search">
          <input
            data-autofocus
            aria-label="Szukaj pola"
            placeholder="Szukaj nazwy, ścieżki lub wartości…"
            value={search}
            onChange={(event) => setSearch(event.target.value)}
          />
        </div>
        <div className="af-response-field-list" aria-label="Pola odpowiedzi">
          {visible.map((field) => (
            <button
              type="button"
              key={field.path}
              className={field.path === value ? 'is-selected' : ''}
              aria-pressed={field.path === value}
              title={`Wybierz ${field.path || `całe ${port}`}`}
              onClick={() => {
                onChange(field.path);
                onClose();
              }}
            >
              <code style={{ paddingLeft: Math.min(field.depth, 6) * 10 }}>
                {field.path || `Całe ${port}`}
              </code>
              <small>{types[field.type]}</small>
              <span>{field.preview}</span>
            </button>
          ))}
          {!visible.length && (
            <p className="af-field-picker-empty">
              {sample === undefined
                ? 'Wykonaj request lub dodaj przykładową odpowiedź. Możesz też wpisać własną ścieżkę.'
                : 'Nie znaleziono pola. Zmień wyszukiwanie lub wpisz własną ścieżkę w argumencie.'}
            </p>
          )}
        </div>
        <footer>
          <span>
            {visible.length} pól{fields.length === 1000 ? ' · pokazano pierwsze 1000' : ''}. Wybór
            pola ustawia powiązanie, nie wartość oczekiwaną.
          </span>
          <button type="button" onClick={onClose}>
            Anuluj
          </button>
        </footer>
      </div>
    </div>,
    document.body,
  );
}
export function ResponseFieldPicker(props: Props) {
  const [open, setOpen] = useState(false);
  const selected = props.sample === undefined ? undefined : getPath(props.sample, props.value);
  const preview = collectResponseFields(selected as Json | undefined, 1)[0];
  return (
    <div className="af-response-field-picker">
      <button
        type="button"
        className="af-response-field-trigger"
        aria-label="Wybierz pole z odpowiedzi"
        onClick={() => setOpen(true)}
      >
        <span>Pole odpowiedzi</span>
        <strong>{props.value || `Całe ${props.port}`}</strong>
        <span>Wybierz pole… ▾</span>
      </button>
      <small>
        {props.origin}
        {preview ? ` · ${types[preview.type]}: ${preview.preview}` : ''}
      </small>
      {open && <ResponseFieldDialog {...props} onClose={() => setOpen(false)} />}
    </div>
  );
}

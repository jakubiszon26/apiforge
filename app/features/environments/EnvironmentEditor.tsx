import { useState } from 'react';
import type { Environment } from '../../../shared/model';
import { StringMap } from '../inspector/fields';
import '../inspector/inspector.css';
import { useModalFocus } from '../inspector/useModalFocus';

export interface EnvironmentEditorProps {
  environment: Environment;
  secrets: Record<string, string>;
  onChange: (environment: Environment) => void;
  onSecretsChange: (secrets: Record<string, string>) => void;
  onClose: () => void;
}
export function EnvironmentEditor({
  environment,
  secrets,
  onChange,
  onSecretsChange,
  onClose,
}: EnvironmentEditorProps) {
  const modalRef = useModalFocus<HTMLElement>(onClose);
  const [draft, setDraft] = useState<Environment>(() => structuredClone(environment));
  const [secretValues, setSecretValues] = useState({ ...secrets });
  const [name, setName] = useState('');
  let error = '';
  try {
    if (draft.baseUrl && !['http:', 'https:'].includes(new URL(draft.baseUrl).protocol))
      error = 'Base URL musi używać HTTP lub HTTPS.';
  } catch {
    error = 'Wpisz pełny base URL, np. http://localhost:3000.';
  }
  if (Object.keys(draft.variables).some((k) => draft.secretNames.includes(k)))
    error = 'Zmienna i sekret nie mogą mieć tej samej nazwy.';
  if (Object.hasOwn(draft.variables, 'baseUrl') || draft.secretNames.includes('baseUrl'))
    error = 'Nazwa baseUrl jest zarezerwowana dla adresu środowiska.';
  const save = () => {
    if (error) return;
    onChange(draft);
    onSecretsChange(Object.fromEntries(draft.secretNames.map((n) => [n, secretValues[n] ?? ''])));
    onClose();
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
        className="af-modal af-editor af-environment-editor"
        role="dialog"
        aria-modal="true"
        aria-labelledby="af-env-title"
      >
        <header className="af-modal-header">
          <div>
            <span className="af-eyebrow">Ustawienia projektu</span>
            <h2 id="af-env-title">Środowisko testowe</h2>
          </div>
          <button type="button" aria-label="Zamknij" onClick={onClose}>
            ×
          </button>
        </header>
        <div className="af-modal-body">
          <label className="af-field">
            Base URL
            <input
              data-autofocus
              placeholder="http://localhost:3000"
              value={draft.baseUrl}
              onChange={(e) => setDraft({ ...draft, baseUrl: e.target.value })}
            />
          </label>
          <p className="af-hint">
            Wspólny początek adresu dla requestów ze ścieżką względną. Request z pełnym URL korzysta
            z własnego adresu.
          </p>
          <StringMap
            label="Zmienne środowiska"
            value={draft.variables}
            onChange={(variables) => setDraft({ ...draft, variables })}
          />
          <div className="af-section-title">
            Sekrety sesyjne <span>tylko w pamięci</span>
          </div>
          <p className="af-hint">
            Projekt zapisuje nazwy sekretów. Wartości znikają po odświeżeniu strony i nie trafiają
            do eksportu projektu.
          </p>
          {draft.secretNames.map((n) => (
            <div className="af-map-row" key={n}>
              <code>{n}</code>
              <input
                type="password"
                autoComplete="off"
                aria-label={`Sekret ${n}`}
                value={secretValues[n] ?? ''}
                placeholder="Wartość na tę sesję"
                onChange={(e) => setSecretValues({ ...secretValues, [n]: e.target.value })}
              />
              <button
                type="button"
                className="af-icon-button"
                aria-label={`Usuń sekret ${n}`}
                onClick={() =>
                  setDraft({ ...draft, secretNames: draft.secretNames.filter((k) => k !== n) })
                }
              >
                ×
              </button>
            </div>
          ))}
          <div className="af-inline">
            <input
              aria-label="Nazwa nowego sekretu"
              placeholder="np. apiToken"
              value={name}
              onChange={(e) => setName(e.target.value)}
            />
            <button
              type="button"
              disabled={
                !name.trim() ||
                name.trim() === 'baseUrl' ||
                draft.secretNames.includes(name.trim()) ||
                Object.hasOwn(draft.variables, name.trim())
              }
              onClick={() => {
                setDraft({ ...draft, secretNames: [...draft.secretNames, name.trim()] });
                setName('');
              }}
            >
              Dodaj sekret
            </button>
          </div>
          <p className="af-hint">
            Aby użyć sekretu, ustaw źródło argumentu klocka na „Środowisko / sekret” i wybierz jego
            nazwę.
          </p>
          {error && <p className="af-error">{error}</p>}
        </div>
        <footer className="af-modal-footer">
          <button type="button" onClick={onClose}>
            Anuluj
          </button>
          <button type="button" className="af-primary" disabled={Boolean(error)} onClick={save}>
            Zapisz środowisko
          </button>
        </footer>
      </section>
    </div>
  );
}

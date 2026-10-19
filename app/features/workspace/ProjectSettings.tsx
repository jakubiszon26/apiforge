import { useState } from 'react';
import type { Contract, Json, Project, Scenario } from '../../../shared/model';
import { uid } from '../../../shared/model';
import { JsonEditor } from '../inspector/fields';
import './project-settings.css';

export function ProjectSettings({
  project,
  scenario,
  onChange,
  onClose,
}: {
  project: Project;
  scenario: Scenario;
  onChange: (project: Project) => void;
  onClose: () => void;
}) {
  const [tab, setTab] = useState<'scenario' | 'contracts'>('scenario');
  const [parameterError, setParameterError] = useState('');
  const updateScenario = (patch: Partial<Scenario>) =>
    onChange({
      ...project,
      scenarios: project.scenarios.map((s) => (s.id === scenario.id ? { ...s, ...patch } : s)),
    });
  const updateContract = (id: string, patch: Partial<Contract>) =>
    onChange({
      ...project,
      contracts: project.contracts.map((c) => (c.id === id ? { ...c, ...patch } : c)),
    });
  return (
    <div className="project-settings-backdrop">
      <section
        className="project-settings-modal"
        role="dialog"
        aria-modal="true"
        aria-label="Ustawienia projektu"
      >
        <header>
          <h2>Ustawienia projektu</h2>
          <button onClick={onClose} aria-label="Zamknij ustawienia">
            ×
          </button>
        </header>
        <nav>
          <button className={tab === 'scenario' ? 'active' : ''} onClick={() => setTab('scenario')}>
            Projekt i scenariusz
          </button>
          <button
            className={tab === 'contracts' ? 'active' : ''}
            onClick={() => setTab('contracts')}
          >
            Kontrakty danych
          </button>
        </nav>
        {tab === 'scenario' ? (
          <div className="project-settings-content">
            <label>
              Nazwa projektu
              <input
                value={project.name}
                onChange={(e) => onChange({ ...project, name: e.target.value })}
              />
            </label>
            <label>
              Nazwa scenariusza
              <input
                value={scenario.name}
                onChange={(e) => updateScenario({ name: e.target.value })}
              />
            </label>
            <label>
              Opis
              <textarea
                value={scenario.description}
                onChange={(e) => updateScenario({ description: e.target.value })}
              />
            </label>
            <label>
              Po błędzie
              <select
                value={scenario.errorPolicy}
                onChange={(e) =>
                  updateScenario({ errorPolicy: e.target.value as Scenario['errorPolicy'] })
                }
              >
                <option value="continue">Kontynuuj możliwe kroki</option>
                <option value="stop">Zatrzymaj test</option>
              </select>
            </label>
            <JsonEditor
              label="Parametry scenariusza (obiekt JSON)"
              value={scenario.parameters}
              onChange={(v) => {
                if (v && typeof v === 'object' && !Array.isArray(v)) {
                  updateScenario({ parameters: v as Record<string, Json> });
                  setParameterError('');
                } else setParameterError('Parametry muszą być obiektem JSON.');
              }}
            />
            {parameterError && <p role="alert">{parameterError}</p>}
          </div>
        ) : (
          <div className="project-settings-content">
            <p>
              Kontrakty nazywają dane projektu i decydują, które własne akcje pojawią się przy
              wyjściu komponentu.
            </p>
            {project.contracts.map((c) => (
              <div className="project-settings-contract" key={c.id}>
                <label>
                  Nazwa kontraktu
                  <input
                    value={c.name}
                    onChange={(e) => updateContract(c.id, { name: e.target.value })}
                  />
                </label>
                <small>ID: {c.id}</small>
                <JsonEditor
                  label="Przykładowa struktura"
                  value={c.sample}
                  onChange={(sample) => updateContract(c.id, { sample })}
                />
              </div>
            ))}
            <button
              onClick={() =>
                onChange({
                  ...project,
                  contracts: [
                    ...project.contracts,
                    { id: uid('contract'), name: 'Nowy kontrakt', sample: {} },
                  ],
                })
              }
            >
              + Dodaj kontrakt
            </button>
          </div>
        )}
        <footer>
          <button onClick={onClose}>Gotowe</button>
        </footer>
      </section>
    </div>
  );
}

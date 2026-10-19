import { reference } from '../../../shared/model';
import { writeVariable } from '../../../shared/variables';
import { supportedKey } from '../inspector/jsonPaths';
import { useEffect, useMemo, useState } from 'react';
import {
  IconArrowRight,
  IconCheck,
  IconChevronDown,
  IconChevronRight,
  IconClock,
  IconDownload,
  IconFlask,
  IconPlayerPlay,
  IconPlus,
  IconX,
} from '@tabler/icons-react';
import type { Json, RunResult, StepResult } from '../../../shared/model';
import { appendPath } from '../../../shared/graph';
import { redactReport } from './redact';
import './runs.css';

export interface RunPanelProps {
  run: RunResult | null;
  selectedNodeId?: string;
  onSelect: (id: string) => void;
  onAssertField: (nodeId: string, port: string, path: string, value: Json) => void;
  onAssignField?: (nodeId: string, port: string, path: string) => void;
}
const statusLabels = {
  passed: 'Zaliczony',
  failed: 'Niezgodność',
  error: 'Błąd wykonania',
  blocked: 'Zablokowany',
  skipped: 'Pominięty',
  cancelled: 'Anulowany',
};
interface Row {
  step: StepResult;
  key: string;
  rootId: string;
  depth: number;
}
function rows(steps: StepResult[], rootId?: string, prefix = '', depth = 0): Row[] {
  return steps.flatMap((step, index) => {
    const key = `${prefix}${index}:${step.nodeId}`;
    return [
      { step, key, rootId: rootId ?? step.nodeId, depth },
      ...rows(step.children ?? [], rootId ?? step.nodeId, `${key}/`, depth + 1),
    ];
  });
}
const describe = (value: Json) =>
  value === null
    ? 'null'
    : Array.isArray(value)
      ? `Lista (${value.length})`
      : typeof value === 'object'
        ? `Obiekt (${Object.keys(value).length})`
        : JSON.stringify(value);
function ValueTree({
  value,
  path = '',
  label,
  onAssert,
  onAssign,
  onDragField,
  depth = 0,
  validPath = true,
}: {
  value: Json;
  path?: string;
  label: string;
  onAssert?: (path: string, value: Json) => void;
  onAssign?: (path: string) => void;
  onDragField?: (path: string, data: DataTransfer) => void;
  depth?: number;
  validPath?: boolean;
}) {
  const expandable = value !== null && typeof value === 'object';
  const [open, setOpen] = useState(depth < 1);
  const entries = expandable ? Object.entries(value) : [];
  return (
    <div className="af-value-branch">
      <div className="af-value-row" style={{ paddingLeft: 8 + depth * 14 }}>
        <button
          className="af-tree-toggle"
          disabled={!expandable}
          onClick={() => setOpen(!open)}
          aria-label={`${open ? 'Zwiń' : 'Rozwiń'} ${label}`}
          aria-expanded={expandable ? open : undefined}
        >
          {expandable ? (
            open ? (
              <IconChevronDown size={12} />
            ) : (
              <IconChevronRight size={12} />
            )
          ) : (
            <span>·</span>
          )}
        </button>
        <span
          className={`af-json-key ${onDragField && validPath ? 'af-draggable-response-field' : ''}`}
          title={path || '(cała wartość)'}
          draggable={Boolean(onDragField) && validPath}
          onDragStart={(event) => onDragField?.(path, event.dataTransfer)}
        >
          {label}
        </span>
        <span className="af-json-colon">:</span>
        <code
          className={`af-json-value af-json-${value === null ? 'null' : typeof value}`}
          title={describe(value)}
        >
          {describe(value)}
        </code>
        {onAssert && validPath && (
          <button
            className="af-field-assert"
            title={`Sprawdź ${path || label}`}
            onClick={() => onAssert(path, value)}
          >
            <IconPlus size={11} /> Sprawdź
          </button>
        )}
        {onAssign && validPath && (
          <button
            className="af-field-assert"
            title={`Przypisz ${path || label} do zmiennej`}
            onClick={() => onAssign(path)}
          >
            Do zmiennej
          </button>
        )}
        {onAssert && !validPath && (
          <span
            className="af-path-unsupported"
            title="To zastrzeżona nazwa pola. Wybierz obiekt nadrzędny."
          >
            niestandardowy klucz
          </span>
        )}
      </div>
      {expandable &&
        open &&
        entries.map(([key, child]) => (
          <ValueTree
            key={key}
            value={child}
            path={appendPath(path, Array.isArray(value) ? Number(key) : key)}
            label={key}
            onAssert={onAssert}
            onAssign={onAssign}
            onDragField={onDragField}
            depth={depth + 1}
            validPath={validPath && (Array.isArray(value) || supportedKey(key))}
          />
        ))}
    </div>
  );
}
function StatusMark({ status }: { status: StepResult['status'] }) {
  return (
    <span className={`af-run-status-icon af-run-${status}`}>
      {status === 'passed' ? (
        <IconCheck size={12} />
      ) : status === 'failed' || status === 'error' ? (
        <IconX size={12} />
      ) : (
        <span>−</span>
      )}
    </span>
  );
}
export function RunPanel({
  run,
  selectedNodeId,
  onSelect,
  onAssertField,
  onAssignField,
}: RunPanelProps) {
  const flattened = useMemo(() => rows(run?.steps ?? []), [run?.steps]);
  const [selectedKey, setSelectedKey] = useState<string>();
  const [tab, setTab] = useState<'outputs' | 'inputs' | 'request'>('outputs');
  useEffect(() => {
    const item = flattened.find((row) => row.step.nodeId === selectedNodeId && row.depth === 0);
    if (item) setSelectedKey(item.key);
  }, [selectedNodeId, run?.id]);
  const selected =
    flattened.find((row) => row.key === selectedKey) ??
    flattened.find((row) => row.step.nodeId === selectedNodeId) ??
    flattened.find((row) => ['error', 'failed'].includes(row.step.status)) ??
    flattened[0];
  const step = selected?.step;
  useEffect(() => {
    if (tab === 'request' && !step?.request) setTab('outputs');
  }, [tab, step?.request]);
  const exportReport = () => {
    if (!run) return;
    const blob = new Blob([JSON.stringify(redactReport(run), null, 2)], {
      type: 'application/json',
    });
    const url = URL.createObjectURL(blob),
      anchor = document.createElement('a');
    anchor.href = url;
    anchor.download = `api-forge-run-${run.id}.json`;
    anchor.click();
    setTimeout(() => URL.revokeObjectURL(url), 1000);
  };
  const count = (status: StepResult['status']) =>
    run?.steps.filter((s) => s.status === status).length ?? 0;
  return (
    <section className="af-run-panel" aria-label="Wyniki wykonania">
      <header className="af-run-header">
        <div className="af-run-title">
          <IconPlayerPlay size={14} />
          <strong>Przebieg testu</strong>
          {run && (
            <span className={`af-run-mode ${run.mode}`}>
              {run.mode === 'simulation' ? (
                <>
                  <IconFlask size={11} /> SYMULACJA
                </>
              ) : (
                'RZECZYWISTE API'
              )}
            </span>
          )}
        </div>
        <div className="af-run-summary">
          {run && (
            <>
              <span className="af-summary-passed">{count('passed')} zaliczonych</span>
              {count('failed') + count('error') > 0 && (
                <span className="af-summary-failed">{count('failed') + count('error')} błędów</span>
              )}
              <button
                onClick={exportReport}
                title="Eksportuj raport JSON z zamaskowanymi sekretami"
              >
                <IconDownload size={13} /> Eksport
              </button>
            </>
          )}
        </div>
      </header>
      {!run ? (
        <div className="af-run-empty">
          <span>
            <IconPlayerPlay size={24} stroke={1.4} />
          </span>
          <div>
            <strong>Twój scenariusz jest gotowy na pierwszy przebieg</strong>
            <p>
              Uruchom symulację lub rzeczywiste API. Tutaj zobaczysz dane każdego kroku i wyniki
              porównań.
            </p>
          </div>
        </div>
      ) : (
        <>
          <div className="af-run-meta">
            <span>
              <IconClock size={11} /> {new Date(run.startedAt).toLocaleTimeString('pl-PL')}
              {run.finishedAt &&
                ` · ${Math.max(0, new Date(run.finishedAt).getTime() - new Date(run.startedAt).getTime())} ms`}
            </span>
            <span>
              {run.status === 'running'
                ? 'Wykonywanie…'
                : run.status === 'passed'
                  ? 'Zakończony poprawnie'
                  : run.status === 'cancelled'
                    ? 'Anulowany'
                    : 'Zakończony z błędami'}
            </span>
            {run.diagnostic && (
              <strong>
                Przebieg diagnostyczny · może korzystać z zapisanych wejść · nie zalicza pełnego
                testu
              </strong>
            )}
            {run.mode === 'simulation' && (
              <span>Odpowiedzi przykładowe — bez wysyłania requestów</span>
            )}
          </div>
          {run.error && (
            <div className="af-run-error" role="alert">
              {run.error}
            </div>
          )}
          <div className="af-run-body">
            <nav className="af-run-steps" aria-label="Kroki przebiegu">
              {flattened.length === 0 && <p>Oczekiwanie na pierwszy krok…</p>}
              {flattened.map((row, index) => (
                <div key={row.key}>
                  {row.step.phase === 'cleanup' &&
                    row.depth === 0 &&
                    (index === 0 || flattened[index - 1].step.phase !== 'cleanup') && (
                      <div className="af-cleanup-label">PO TEŚCIE · SPRZĄTANIE</div>
                    )}
                  <button
                    className={`af-run-step-row ${selected?.key === row.key ? 'is-active' : ''}`}
                    style={{ paddingLeft: 14 + row.depth * 14 }}
                    onClick={() => {
                      setSelectedKey(row.key);
                      onSelect(row.rootId);
                    }}
                  >
                    <StatusMark status={row.step.status} />
                    <span>
                      <strong>{row.step.name}</strong>
                      <small>
                        {row.depth > 0 ? 'Wewnątrz komponentu · ' : ''}
                        {statusLabels[row.step.status]}
                      </small>
                    </span>
                    <time>{row.step.durationMs} ms</time>
                  </button>
                </div>
              ))}
            </nav>
            <div className="af-step-details">
              {step && selected ? (
                <>
                  <div className="af-step-detail-head">
                    <span>
                      <strong>{step.name}</strong>
                      {selected.depth > 0 && (
                        <small>Krok wewnątrz komponentu · {step.nodeId}</small>
                      )}
                    </span>
                    <span className={`af-detail-status af-run-${step.status}`}>
                      {statusLabels[step.status]}
                    </span>
                  </div>
                  {step.message && (
                    <div className={`af-step-message af-message-${step.status}`}>
                      {step.message}
                    </div>
                  )}
                  {(Object.hasOwn(step, 'expected') || Object.hasOwn(step, 'actual')) && (
                    <div className="af-comparison">
                      <div>
                        <small>OCZEKIWANO</small>
                        <code>
                          {Object.hasOwn(step, 'expected')
                            ? JSON.stringify(step.expected, null, 2)
                            : '—'}
                        </code>
                      </div>
                      <IconArrowRight size={15} />
                      <div>
                        <small>OTRZYMANO</small>
                        <code>
                          {Object.hasOwn(step, 'actual')
                            ? JSON.stringify(step.actual, null, 2)
                            : '(brak wartości)'}
                        </code>
                      </div>
                    </div>
                  )}
                  <div className="af-result-tabs">
                    <button
                      className={tab === 'outputs' ? 'active' : ''}
                      onClick={() => setTab('outputs')}
                    >
                      Wyjścia <small>{Object.keys(step.outputs).length}</small>
                    </button>
                    <button
                      className={tab === 'inputs' ? 'active' : ''}
                      onClick={() => setTab('inputs')}
                    >
                      Wejścia
                    </button>
                    {step.request && (
                      <button
                        className={tab === 'request' ? 'active' : ''}
                        onClick={() => setTab('request')}
                      >
                        Request HTTP
                      </button>
                    )}
                    <span>
                      {selected.depth === 0 && tab === 'outputs'
                        ? 'Wybierz pole → dodaj sprawdzenie'
                        : ''}
                    </span>
                  </div>
                  <div className="af-json-tree">
                    {tab === 'request' && step.request ? (
                      <>
                        <div className="af-report-request">
                          <b>{step.request.method}</b>
                          <code>{step.request.url}</code>
                        </div>
                        <ValueTree label="headers" value={step.request.headers} />
                        {step.request.body !== undefined && (
                          <ValueTree label="body" value={step.request.body} />
                        )}
                      </>
                    ) : (
                      Object.entries(tab === 'inputs' ? step.inputs : step.outputs).map(
                        ([port, value]) => (
                          <ValueTree
                            key={`${run.id}-${selected.key}-${tab}-${port}`}
                            label={port}
                            value={value}
                            onAssign={
                              tab === 'outputs' && selected.depth === 0 && onAssignField
                                ? (path) => onAssignField(step.nodeId, port, path)
                                : undefined
                            }
                            onDragField={
                              tab === 'outputs' && selected.depth === 0
                                ? (path, data) =>
                                    writeVariable(data, {
                                      label: (path || port).slice(0, 200),
                                      binding: reference(step.nodeId, port, path),
                                    })
                                : undefined
                            }
                            onAssert={
                              tab === 'outputs' && selected.depth === 0
                                ? (path, value) => onAssertField(step.nodeId, port, path, value)
                                : undefined
                            }
                          />
                        ),
                      )
                    )}
                    {Object.keys(tab === 'inputs' ? step.inputs : step.outputs).length === 0 &&
                      tab !== 'request' && (
                        <p className="af-no-data">
                          Brak {tab === 'inputs' ? 'wejść' : 'wyjść'} dla tego kroku.
                        </p>
                      )}
                  </div>
                </>
              ) : (
                <p className="af-no-data">Wyniki pojawią się podczas wykonania.</p>
              )}
            </div>
          </div>
        </>
      )}
    </section>
  );
}

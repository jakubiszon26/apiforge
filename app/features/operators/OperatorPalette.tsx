import { operators, type Operator } from '../../../shared/operators';
import './operators.css';
export const OPERATOR_MIME = 'application/apiforge-operator';
export function OperatorPalette({
  search = '',
  onAdd,
  onExample,
}: {
  search?: string;
  onAdd: (operator: Operator) => void;
  onExample: () => void;
}) {
  return (
    <div className="af-operator-palette">
      <button className="af-operator-example" onClick={onExample}>
        Otwórz przykład blokowy
      </button>
      <p className="af-library-category">OPERATORY · {Object.keys(operators).length}</p>
      <p className="af-operator-help">
        Przeciągnij do argumentu lub na graf. Kliknięcie dodaje krok.
      </p>
      {[...new Set(Object.values(operators).map((operator) => operator.group))].map((group) => {
        const entries = Object.entries(operators).filter(
          ([, operator]) =>
            operator.group === group &&
            operator.label.toLocaleLowerCase('pl').includes(search.toLocaleLowerCase('pl')),
        );
        return entries.length ? (
          <details key={`${group}:${Boolean(search)}`} open={Boolean(search) || undefined}>
            <summary>
              {group} <small>{entries.length}</small>
            </summary>
            {entries.map(([name, operator]) => (
              <button
                type="button"
                draggable
                className={`af-operator-chip ${operator.output === 'boolean' ? 'is-boolean' : ''}`}
                key={name}
                onClick={() => onAdd(name as Operator)}
                onDragStart={(event) => {
                  event.dataTransfer.setData(OPERATOR_MIME, name);
                  event.dataTransfer.setData(
                    'application/apiforge-node',
                    JSON.stringify({ kind: 'formula', operation: name }),
                  );
                  event.dataTransfer.effectAllowed = 'copy';
                }}
              >
                {operator.label}
                <small>{operator.output === 'boolean' ? 'tak / nie' : operator.output}</small>
              </button>
            ))}
          </details>
        ) : null;
      })}
    </div>
  );
}

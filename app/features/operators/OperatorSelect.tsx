import { operators, type Operator } from '../../../shared/operators';
export function OperatorSelect({
  value,
  onChange,
}: {
  value: Operator;
  onChange: (operator: Operator) => void;
}) {
  return (
    <select
      aria-label="Operator"
      value={value}
      onChange={(event) => onChange(event.target.value as Operator)}
    >
      {[...new Set(Object.values(operators).map((operator) => operator.group))].map((group) => (
        <optgroup key={group} label={group}>
          {Object.entries(operators)
            .filter(([, operator]) => operator.group === group)
            .map(([name, operator]) => (
              <option key={name} value={name}>
                {operator.label}
              </option>
            ))}
        </optgroup>
      ))}
    </select>
  );
}

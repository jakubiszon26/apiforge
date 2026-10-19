import { writeVariable, type VariableBlock } from '../../../shared/variables';

export function VariableChip({
  variable,
  onRemove,
}: {
  variable: VariableBlock;
  onRemove?: () => void;
}) {
  return (
    <span className="af-bound-variable">
      <span
        draggable
        onDragStart={(event) => writeVariable(event.dataTransfer, variable)}
        title="Przeciągnij zmienną do innego pola"
      >
        {variable.label}
      </span>
      {onRemove && (
        <button type="button" aria-label="Usuń powiązanie zmiennej" onClick={onRemove}>
          ×
        </button>
      )}
    </span>
  );
}

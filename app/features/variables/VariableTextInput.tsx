import type { Binding } from '../../../shared/model';
import { VariableChip } from './VariableChip';
import { createContext, useContext, useState, type InputHTMLAttributes } from 'react';
import {
  insertVariableToken,
  readVariable,
  VARIABLE_MIME,
  type VariableBlock,
  type TextSelection,
} from '../../../shared/variables';

export const VariableBindingsContext = createContext<Record<string, Binding>>({});
interface Props extends Omit<InputHTMLAttributes<HTMLInputElement>, 'onChange'> {
  onValueChange: (value: string) => void;
  onVariableDrop?: (variable: VariableBlock, selection: TextSelection) => void;
}
/** Preserve the selected range; normal typing and variable drops share one controlled value. */
export function VariableTextInput({ onValueChange, onVariableDrop, className, ...props }: Props) {
  const [over, setOver] = useState(false);
  const bindings = useContext(VariableBindingsContext);
  const tokens = [
    ...new Set(
      [...String(props.value ?? '').matchAll(/\{\{\s*([^{}]+?)\s*\}\}/g)].map((match) => match[1]),
    ),
  ];
  return (
    <span className="af-variable-text-field">
      <input
        {...props}
        className={`${className ?? ''} ${over ? 'af-variable-drop-over' : ''}`}
        onChange={(event) => onValueChange(event.target.value)}
        onDragOver={(event) => {
          if (event.dataTransfer.types.includes(VARIABLE_MIME)) {
            event.preventDefault();
            event.stopPropagation();
            setOver(true);
          }
        }}
        onDragLeave={() => setOver(false)}
        onDrop={(event) => {
          const variable = readVariable(event.dataTransfer);
          if (!variable) return;
          event.preventDefault();
          event.stopPropagation();
          setOver(false);
          const input = event.currentTarget;
          const selection = {
            value: input.value,
            start: input.selectionStart ?? input.value.length,
            end: input.selectionEnd ?? input.value.length,
          };
          if (onVariableDrop) onVariableDrop(variable, selection);
          else onValueChange(insertVariableToken(selection, variable.label));
        }}
      />
      {tokens.length > 0 && (
        <span className="af-text-variable-tokens">
          {tokens.map((name) => {
            const binding = bindings[name];
            return binding && binding.kind !== 'literal' && binding.kind !== 'expression' ? (
              <VariableChip key={name} variable={{ label: name, binding }} />
            ) : (
              <span key={name} className="af-text-variable-name">
                {name}
              </span>
            );
          })}
        </span>
      )}
    </span>
  );
}

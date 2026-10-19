import type { Graph, Json, Project, TestNode } from '../../../shared/model';
import { changeOperator, operators, type Expression } from '../../../shared/operators';
import { BindingEditor } from '../inspector/BindingEditor';
import { OperatorSelect } from './OperatorSelect';
import './operators.css';

/** Each slot contains another Binding; editing never generates executable source code. */
export function ExpressionBlock({
  value,
  onChange,
  onUnwrap,
  node,
  graph,
  project,
  parameters,
  depth,
}: {
  value: Expression;
  onChange: (value: Expression) => void;
  onUnwrap: () => void;
  node: TestNode;
  graph: Graph;
  project: Project;
  parameters: Record<string, Json>;
  depth: number;
}) {
  const operator = operators[value.operator];
  return (
    <section
      className={`af-expression ${operator.output === 'boolean' ? 'is-boolean' : ''}`}
      data-operator={value.operator}
      aria-label={`Blok ${operator.label}`}
    >
      <header>
        <span>{operator.group}</span>
        <b>{operator.output === 'boolean' ? 'warunek' : 'wartość'}</b>
        <button
          type="button"
          className="af-operator-unwrap"
          title="Usuń ten operator, zachowaj argument A"
          aria-label="Usuń operator"
          onClick={onUnwrap}
        >
          ×
        </button>
      </header>
      <OperatorSelect
        value={value.operator}
        onChange={(name) => onChange(changeOperator(value, name))}
      />
      <div className="af-expression-slots">
        {operator.inputs.map((port) => (
          <div className="af-expression-slot" key={port.name}>
            <BindingEditor
              value={value.inputs[port.name]}
              port={port}
              node={node}
              graph={graph}
              project={project}
              parameters={parameters}
              depth={depth + 1}
              onChange={(binding) => {
                const inputs = { ...value.inputs };
                if (binding) inputs[port.name] = binding;
                else delete inputs[port.name];
                onChange({ ...value, inputs });
              }}
            />
          </div>
        ))}
      </div>
    </section>
  );
}

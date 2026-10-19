import type { Binding } from './model';

type Leaf = Exclude<Binding, { kind: 'expression' }>;
export function bindingLeaves(
  binding: Binding,
  path: string[] = [],
): { binding: Leaf; path: string[] }[] {
  return binding.kind === 'expression'
    ? Object.entries(binding.inputs).flatMap(([name, child]) =>
        bindingLeaves(child, [...path, name]),
      )
    : [{ binding, path }];
}
export function references(binding: Binding): Extract<Binding, { kind: 'reference' }>[] {
  return bindingLeaves(binding).flatMap(({ binding }) =>
    binding.kind === 'reference' ? [binding] : [],
  );
}
/** Returning undefined removes a leaf slot, preserving the operator around it. */
export function mapBinding(
  binding: Binding,
  visit: (binding: Leaf, path: string[]) => Binding | undefined,
  path: string[] = [],
): Binding | undefined {
  if (binding.kind !== 'expression') return visit(binding, path);
  return {
    ...binding,
    inputs: Object.fromEntries(
      Object.entries(binding.inputs).flatMap(([name, child]) => {
        const next = mapBinding(child, visit, [...path, name]);
        return next ? [[name, next]] : [];
      }),
    ),
  };
}
export function replaceBindingAt(binding: Binding, path: string[], value: Binding): Binding {
  if (!path.length) return value;
  if (binding.kind !== 'expression') throw new Error('Nieprawidłowa ścieżka argumentu operatora.');
  const [name, ...rest] = path;
  return {
    ...binding,
    inputs: { ...binding.inputs, [name]: replaceBindingAt(binding.inputs[name], rest, value) },
  };
}
export const bindingEdgeId = (nodeId: string, input: string, path: string[]) =>
  `data:${nodeId}:${input}${path.length ? `:${path.join('/')}` : ''}`;

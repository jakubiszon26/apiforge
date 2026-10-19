import { createNode } from '../../../shared/graph';
import { literal, reference } from '../../../shared/model';
import type { Json, TestNode } from '../../../shared/model';

/** Keep the source binding identical for assertions made from samples and run results. */
export function createFieldAssertion(
  source: TestNode,
  port: string,
  path: string,
  value: Json,
): TestNode {
  const node = createNode('assert', { x: source.position.x + 340, y: source.position.y + 70 });
  node.name = `Sprawdź ${path || port}`;
  node.inputs = {
    actual: reference(source.id, port, path),
    expected: literal(structuredClone(value)),
  };
  return node;
}

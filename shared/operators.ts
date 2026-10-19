import type { Binding, Json, Port, ValueType } from './model';

const number = (name: string, label: string, defaultValue = 0): Port => ({
  name,
  label,
  type: 'number',
  required: true,
  defaultValue,
});
const text = (name: string, label: string, defaultValue = ''): Port => ({
  name,
  label,
  type: 'string',
  required: true,
  defaultValue,
});
const any = (name: string, label: string): Port => ({
  name,
  label,
  type: 'any',
  required: true,
  defaultValue: null,
});
const bool = (name: string, label: string): Port => ({
  name,
  label,
  type: 'boolean',
  required: true,
  defaultValue: false,
});
interface OperatorDefinition {
  label: string;
  group: string;
  output: ValueType;
  inputs: Port[];
}
const define = (
  label: string,
  group: string,
  output: ValueType,
  inputs: Port[],
): OperatorDefinition => ({ label, group, output, inputs });
const pair = [number('left', 'A'), number('right', 'B', 1)];
const unary = [number('left', 'Wartość')];

/** One catalog drives the editor, graph ports, validation and interpreter. */
export const operators = {
  add: define('A + B', 'Matematyka', 'number', pair),
  subtract: define('A − B', 'Matematyka', 'number', pair),
  multiply: define('A × B', 'Matematyka', 'number', pair),
  divide: define('A ÷ B', 'Matematyka', 'number', pair),
  modulo: define('A modulo B', 'Matematyka', 'number', pair),
  random: define('Losuj od A do B', 'Matematyka', 'number', pair),
  round: define('Zaokrąglij', 'Matematyka', 'number', [
    number('left', 'Wartość'),
    number('right', 'Miejsca dziesiętne'),
  ]),
  equals: define('A = B', 'Porównania', 'boolean', [any('left', 'A'), any('right', 'B')]),
  notEquals: define('A ≠ B', 'Porównania', 'boolean', [any('left', 'A'), any('right', 'B')]),
  greater: define('A > B', 'Porównania', 'boolean', pair),
  less: define('A < B', 'Porównania', 'boolean', pair),
  atLeast: define('A ≥ B', 'Porównania', 'boolean', pair),
  atMost: define('A ≤ B', 'Porównania', 'boolean', pair),
  and: define('A i B', 'Logika', 'boolean', [bool('left', 'A'), bool('right', 'B')]),
  or: define('A lub B', 'Logika', 'boolean', [bool('left', 'A'), bool('right', 'B')]),
  not: define('Nie A', 'Logika', 'boolean', [bool('left', 'A')]),
  concat: define('Połącz teksty', 'Tekst', 'string', [any('left', 'A'), any('right', 'B')]),
  letter: define('Litera tekstu', 'Tekst', 'string', [
    text('left', 'Tekst'),
    number('right', 'Numer litery', 1),
  ]),
  length: define('Długość tekstu / listy', 'Tekst', 'number', [any('left', 'Tekst lub lista')]),
  contains: define('A zawiera B', 'Tekst', 'boolean', [
    any('left', 'Tekst lub lista'),
    any('right', 'Szukana wartość'),
  ]),
  abs: define('Wartość bezwzględna', 'Funkcje', 'number', unary),
  floor: define('Zaokrąglij w dół', 'Funkcje', 'number', unary),
  ceiling: define('Zaokrąglij w górę', 'Funkcje', 'number', unary),
  sqrt: define('Pierwiastek', 'Funkcje', 'number', unary),
  sin: define('Sinus (stopnie)', 'Funkcje', 'number', unary),
  cos: define('Cosinus (stopnie)', 'Funkcje', 'number', unary),
  tan: define('Tangens (stopnie)', 'Funkcje', 'number', unary),
  asin: define('Arcus sinus', 'Funkcje', 'number', unary),
  acos: define('Arcus cosinus', 'Funkcje', 'number', unary),
  atan: define('Arcus tangens', 'Funkcje', 'number', unary),
  ln: define('Logarytm naturalny', 'Funkcje', 'number', unary),
  log: define('Logarytm dziesiętny', 'Funkcje', 'number', unary),
  exp: define('e do potęgi A', 'Funkcje', 'number', unary),
  power10: define('10 do potęgi A', 'Funkcje', 'number', unary),
  typeOf: define('Typ wartości', 'Dane JSON', 'string', [any('left', 'Wartość')]),
  exists: define('Wartość istnieje', 'Dane JSON', 'boolean', [
    { name: 'left', label: 'Wartość', type: 'any', required: false },
  ]),
  field: define('Pole obiektu / element listy', 'Dane JSON', 'any', [
    any('left', 'Dane'),
    text('right', 'Ścieżka'),
  ]),
} satisfies Record<string, OperatorDefinition>;
export type Operator = keyof typeof operators;
export const isOperator = (value: unknown): value is Operator =>
  typeof value === 'string' && Object.hasOwn(operators, value);
export type Expression = Extract<Binding, { kind: 'expression' }>;
export function expression(operator: Operator): Expression {
  return {
    kind: 'expression',
    operator,
    inputs: Object.fromEntries(
      operators[operator].inputs.map((port) => [
        port.name,
        { kind: 'literal', value: port.defaultValue ?? null },
      ]),
    ),
  };
}
export function changeOperator(value: Expression, operator: Operator): Expression {
  const next = expression(operator);
  // Preserve matching slots, including nested sources, when selecting another operator.
  for (const port of operators[operator].inputs)
    if (Object.hasOwn(value.inputs, port.name)) next.inputs[port.name] = value.inputs[port.name];
  return next;
}
export const jsonType = (value: Json | undefined): string =>
  value === undefined
    ? 'missing'
    : value === null
      ? 'null'
      : Array.isArray(value)
        ? 'array'
        : typeof value;
function equal(a: Json | undefined, b: Json | undefined): boolean {
  if (a === b) return true;
  if (
    a === null ||
    b === null ||
    typeof a !== 'object' ||
    typeof b !== 'object' ||
    Array.isArray(a) !== Array.isArray(b)
  )
    return false;
  const keys = Object.keys(a);
  return (
    keys.length === Object.keys(b).length &&
    keys.every(
      (key) =>
        Object.hasOwn(b, key) &&
        equal((a as Record<string, Json>)[key], (b as Record<string, Json>)[key]),
    )
  );
}

/** Strict JSON semantics: missing values and null never silently become numbers or booleans. */
export function evaluateOperator(
  operator: Operator,
  values: Record<string, Json | undefined>,
  readPath: (value: unknown, path: string) => unknown,
): Json | undefined {
  for (const port of operators[operator].inputs) {
    const value = values[port.name];
    if (value === undefined && !port.required) continue;
    if (
      value === undefined ||
      (port.type !== 'any' && jsonType(value) !== port.type) ||
      (port.type === 'number' && !Number.isFinite(value))
    )
      throw new Error(
        `${operators[operator].label}: argument ${port.label} wymaga ${port.type}; otrzymano ${jsonType(value)}.`,
      );
  }
  const a = values.left,
    b = values.right;
  const n = a as number,
    m = b as number;
  let result: Json | undefined;
  switch (operator) {
    case 'add':
      result = n + m;
      break;
    case 'subtract':
      result = n - m;
      break;
    case 'multiply':
      result = n * m;
      break;
    case 'divide':
      if (m === 0) throw new Error('Dzielenie przez zero.');
      result = n / m;
      break;
    case 'modulo':
      if (m === 0) throw new Error('Modulo przez zero.');
      result = ((n % m) + m) % m;
      break;
    case 'random': {
      const low = Math.min(n, m),
        high = Math.max(n, m);
      result =
        Number.isInteger(low) && Number.isInteger(high)
          ? low + Math.floor(Math.random() * (high - low + 1))
          : low + Math.random() * (high - low);
      break;
    }
    case 'round':
      if (!Number.isInteger(m) || m < 0 || m > 12)
        throw new Error('Precyzja zaokrąglenia: całkowita liczba od 0 do 12.');
      result = Math.round((n + Number.EPSILON) * 10 ** m) / 10 ** m;
      break;
    case 'equals':
      result = equal(a, b);
      break;
    case 'notEquals':
      result = !equal(a, b);
      break;
    case 'greater':
      result = n > m;
      break;
    case 'less':
      result = n < m;
      break;
    case 'atLeast':
      result = n >= m;
      break;
    case 'atMost':
      result = n <= m;
      break;
    case 'and':
      result = Boolean(a && b);
      break;
    case 'or':
      result = Boolean(a || b);
      break;
    case 'not':
      result = !a;
      break;
    case 'concat':
      result = String(a) + String(b);
      break;
    case 'letter':
      if (!Number.isInteger(m) || m < 1)
        throw new Error('Numer litery musi być liczbą całkowitą od 1.');
      result = Array.from(a as string)[m - 1] ?? '';
      break;
    case 'length':
      if (typeof a !== 'string' && !Array.isArray(a))
        throw new Error('Długość wymaga tekstu lub listy.');
      result = typeof a === 'string' ? Array.from(a).length : a.length;
      break;
    case 'contains':
      if (typeof a === 'string' && typeof b === 'string') result = a.includes(b);
      else if (Array.isArray(a)) result = a.some((item) => equal(item, b));
      else throw new Error('Zawieranie wymaga dwóch tekstów albo listy i szukanej wartości.');
      break;
    case 'abs':
      result = Math.abs(n);
      break;
    case 'floor':
      result = Math.floor(n);
      break;
    case 'ceiling':
      result = Math.ceil(n);
      break;
    case 'sqrt':
      result = Math.sqrt(n);
      break;
    case 'sin':
      result = Number(Math.sin((n * Math.PI) / 180).toFixed(10));
      break;
    case 'cos':
      result = Number(Math.cos((n * Math.PI) / 180).toFixed(10));
      break;
    case 'tan':
      if (Math.abs(Math.cos((n * Math.PI) / 180)) < 1e-12)
        throw new Error('Tangens nie jest określony dla tego kąta.');
      result = Math.tan((n * Math.PI) / 180);
      break;
    case 'asin':
      result = (Math.asin(n) * 180) / Math.PI;
      break;
    case 'acos':
      result = (Math.acos(n) * 180) / Math.PI;
      break;
    case 'atan':
      result = (Math.atan(n) * 180) / Math.PI;
      break;
    case 'ln':
      result = Math.log(n);
      break;
    case 'log':
      result = Math.log10(n);
      break;
    case 'exp':
      result = Math.exp(n);
      break;
    case 'power10':
      result = 10 ** n;
      break;
    case 'typeOf':
      result = jsonType(a);
      break;
    case 'exists':
      result = a !== undefined;
      break;
    case 'field':
      result = readPath(a, b as string) as Json | undefined;
      break;
  }
  if (typeof result === 'number' && !Number.isFinite(result))
    throw new Error('Wynik operatora nie jest skończoną liczbą.');
  if (typeof result === 'string' && result.length > 100_000)
    throw new Error('Wynik tekstowy przekracza limit 100000 znaków.');
  return result;
}

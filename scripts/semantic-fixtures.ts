/**
 * Expected results of the TypeScript reference semantics, used by the Rust port as
 * a differential test corpus. Inputs are fixed lists plus a seeded pseudo-random
 * sample, so regeneration is deterministic.
 */
import { getPath } from '../shared/graph';
import type { Json } from '../shared/model';
import { evaluateOperator, operators, type Operator } from '../shared/operators';

type Outcome = { value: Json } | { missing: true } | { error: true };

interface OperatorCase {
  operator: Operator;
  left?: Json;
  right?: Json;
  /** Value returned by Math.random while the case is evaluated. */
  random?: number;
  outcome: Outcome;
}

/** Linear congruential generator (Numerical Recipes constants), 32-bit state. */
function createRng(seed: number): () => number {
  let state = seed >>> 0;
  return () => {
    state = (Math.imul(state, 1664525) + 1013904223) >>> 0;
    return state / 2 ** 32;
  };
}

const NUMBERS: Json[] = [
  0,
  -0,
  1,
  -1,
  2,
  3,
  -3,
  5,
  -5,
  7,
  10,
  12,
  13,
  100,
  0.1,
  0.2,
  0.5,
  -0.5,
  1.005,
  1.5,
  2.5,
  -2.5,
  1 / 3,
  0.30000000000000004,
  123.456,
  1e21,
  1.5e21,
  1e-7,
  1.5e-7,
  1e-6,
  1e308,
  -1e308,
  5e-324,
  9007199254740991,
  9007199254740993,
  30,
  45,
  60,
  90,
  180,
  270,
  360,
  -90,
  0.999,
  2 ** 31,
];
/** Smaller pool for the cross product of two numeric operands. */
const NUMBER_PAIRS: Json[] = [
  0,
  1,
  -1,
  2,
  3,
  -5,
  7,
  0.1,
  0.5,
  1.005,
  -2.5,
  1 / 3,
  1e21,
  1e-7,
  1e308,
  5e-324,
  13,
  90,
];
const TEXTS: Json[] = [
  '',
  'a',
  'abc',
  'Hello',
  'hello',
  'a😀b',
  'zażółć',
  '12',
  '2',
  'true',
  'a,b',
];
const OTHERS: Json[] = [
  null,
  true,
  false,
  [],
  [1, null, [2, 3], 'x'],
  [{ a: 1 }],
  {},
  { a: 1, b: [1, { c: null }] },
  { b: [1, { c: null }], a: 1 },
];
const MIXED: Json[] = [...NUMBERS.slice(0, 16), ...TEXTS, ...OTHERS];
const PATHS: Json[] = ['', 'a', 'b[1].c', 'b.1.c', 'b.length', '["a"]', 'a..b', 'missing', 0];

/** Seeded doubles spread over the whole exponent range, plus small integers. */
function sampleNumbers(count: number, seed: number): number[] {
  const random = createRng(seed);
  const values: number[] = [];
  for (let index = 0; index < count; index++) {
    const mantissa = random() * 10;
    const exponent = Math.floor(random() * 80) - 40;
    const sign = random() < 0.5 ? -1 : 1;
    values.push(
      index % 4 === 0 ? Math.round(random() * 2000 - 1000) : sign * mantissa * 10 ** exponent,
    );
  }
  return values;
}

function outcome(evaluate: () => Json | undefined): Outcome {
  try {
    const value = evaluate();
    return value === undefined ? { missing: true } : { value };
  } catch {
    return { error: true };
  }
}

function withRandom<T>(unit: number, run: () => T): T {
  const original = Math.random;
  Math.random = () => unit;
  try {
    return run();
  } finally {
    Math.random = original;
  }
}

function operandPool(operator: Operator, side: 'left' | 'right'): (Json | undefined)[] {
  const port = operators[operator].inputs.find((input) => input.name === side);
  // Every operator also sees a missing operand and a value of the wrong type.
  const extras: (Json | undefined)[] = [undefined, side === 'left' ? 'text' : null];
  if (!port) return [undefined];
  switch (operator) {
    case 'field':
      return side === 'left' ? [...OTHERS, 'text', undefined] : [...PATHS, undefined];
    case 'letter':
      return side === 'left' ? [...TEXTS, ...extras] : [...NUMBERS.slice(0, 16), ...extras];
  }
  switch (port.type) {
    case 'number':
      return [...(operators[operator].inputs.length > 1 ? NUMBER_PAIRS : NUMBERS), ...extras];
    case 'boolean':
      return [true, false, ...extras, 0];
    case 'string':
      return [...TEXTS, ...extras];
    default:
      return [...MIXED, undefined];
  }
}

function operatorCases(): OperatorCase[] {
  const cases: OperatorCase[] = [];
  const evaluate = (operator: Operator, left?: Json, right?: Json) =>
    evaluateOperator(operator, { left, right }, getPath);
  const push = (operator: Operator, left: Json | undefined, right: Json | undefined) =>
    cases.push({
      operator,
      ...(left === undefined ? {} : { left }),
      ...(right === undefined ? {} : { right }),
      outcome: outcome(() => evaluate(operator, left, right)),
    });

  for (const operator of Object.keys(operators) as Operator[]) {
    if (operator === 'random') continue;
    for (const left of operandPool(operator, 'left'))
      for (const right of operandPool(operator, 'right')) push(operator, left, right);
  }

  // Transcendental functions over the whole double range, to expose libm differences.
  const unary: Operator[] = [
    'sqrt',
    'sin',
    'cos',
    'tan',
    'asin',
    'acos',
    'atan',
    'ln',
    'log',
    'exp',
    'power10',
    'abs',
    'floor',
    'ceiling',
  ];
  const samples = sampleNumbers(400, 0x5eed);
  const units = sampleNumbers(200, 0xa11).map((value) => value % 1);
  for (const operator of unary)
    for (const value of [...samples, ...units]) push(operator, value, undefined);
  for (const value of samples) push('round', value, Math.abs(Math.round(value)) % 13);
  for (const [index, value] of samples.entries()) {
    const divisor = samples[(index * 7 + 3) % samples.length];
    push('divide', value, divisor);
    push('modulo', value, divisor);
    push('multiply', value, divisor);
    push('add', value, divisor);
  }

  const random = createRng(0x7a4d);
  for (const [low, high] of [
    [1, 10],
    [10, 1],
    [7, 7],
    [0, 1.5],
    [-5, 5],
    [0.25, -3.75],
  ] as const) {
    for (const unit of [0, 0.5, 0.999999, random(), random(), random()]) {
      cases.push({
        operator: 'random',
        left: low,
        right: high,
        random: unit,
        outcome: withRandom(unit, () => outcome(() => evaluate('random', low, high))),
      });
    }
  }
  return cases;
}

const PATH_DOCUMENT = {
  a: { b: 'nested' },
  'a.b': { id: 'literal' },
  'items[0]': 123,
  '': false,
  zażółć: 'unicode',
  list: [{ value: 42 }, null, [7, 8]],
  '7': 'numeric key',
  present: null,
  'with "quote"': 'quoted',
  'back\\slash': 'escaped',
};

const PATH_CASES = [
  '',
  'a',
  'a.b',
  'a.b.c',
  '["a.b"].id',
  '["items[0]"]',
  '[""]',
  'zażółć',
  '["za\\u017c\\u00f3\\u0142\\u0107"]',
  'list[0].value',
  'list.0.value',
  'list[1]',
  'list[2][1]',
  'list.2.1',
  'list[3]',
  'list.01',
  'list[01].value',
  'list.first',
  'list.length',
  'list["length"]',
  'list.length.x',
  'list[2].length',
  '[007]',
  '[7]',
  '7',
  'list.[0].value',
  'present',
  'present.x',
  'absent',
  '["with \\"quote\\""]',
  '["back\\\\slash"]',
  '.a',
  'a.',
  'a..b',
  'a]',
  'a[x]',
  'a[1',
  'a[-1]',
  '["x',
  '["x"x]',
  'list[0]value',
  '["\\q"]',
  'a b',
  'a-b',
  '[99999999999999999999999]',
  'constructor',
  'a.constructor',
  '["prototype"]',
  '__proto__',
  'a["b"]',
  'a[ 0 ]',
  'list[0.5]',
  'list[+1]',
  'list[1e0]',
];

export function buildSemanticFixtures(): Map<string, string> {
  const files = new Map<string, string>();
  // One case per line keeps the corpus diffable without pretty-printing every value.
  const lines = (values: unknown[]) =>
    `${values.map((value) => JSON.stringify(value)).join('\n')}\n`;

  files.set('semantics/operators.jsonl', lines(operatorCases()));
  files.set(
    'semantics/paths.jsonl',
    lines([
      { document: PATH_DOCUMENT },
      ...PATH_CASES.map((path) => ({
        path,
        outcome: outcome(() => getPath(PATH_DOCUMENT, path) as Json),
      })),
    ]),
  );
  files.set(
    'semantics/numbers.jsonl',
    lines(
      [...NUMBERS, ...sampleNumbers(1000, 0xc0ffee)].map((value) => ({
        value,
        text: String(value),
      })),
    ),
  );
  return files;
}

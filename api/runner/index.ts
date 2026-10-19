import { planExecution } from './control';
import { evaluateOperator } from '../../shared/operators';
import { references } from '../../shared/bindings';
import { randomUUID } from 'node:crypto';
import { isDeepStrictEqual } from 'node:util';
import type {
  Binding,
  Graph,
  Json,
  Predicate,
  RunRequest,
  RunResult,
  StepResult,
  TestNode,
} from '../../shared/model';
import { definition, dependencies, getPath, inputPorts } from '../../shared/graph';
import { validateRunRequest } from './validation';

type Values = Record<string, Json | undefined>;
type Context = {
  parameters: Values;
  outputs: Map<string, StepResult>;
  signal?: AbortSignal;
  stack: string[];
  phase: 'main' | 'cleanup';
};
const sensitive = /authorization|cookie|password|secret|token|api[-_]?key/i;
const own = (object: object, key: string) => Object.hasOwn(object, key);
const compact = (v: Values): Record<string, Json> =>
  Object.fromEntries(
    Object.entries(v).filter((entry): entry is [string, Json] => entry[1] !== undefined),
  );
function check(
  predicate: Predicate,
  actual: Json | undefined,
  expected: Json | undefined,
  tolerance = 0,
): boolean {
  if (predicate === 'exists') return actual !== undefined;
  if (predicate === 'notEmpty')
    return (
      actual !== undefined &&
      actual !== null &&
      actual !== '' &&
      (!(typeof actual === 'object') || Object.keys(actual).length > 0)
    );
  if (actual === undefined || expected === undefined)
    throw new Error('Brak wartości wymaganej do porównania.');
  switch (predicate) {
    case 'equals':
      return isDeepStrictEqual(actual, expected);
    case 'notEquals':
      return !isDeepStrictEqual(actual, expected);
    // JSON distinguishes null and arrays from ordinary objects.
    case 'type':
      return (
        (actual === null ? 'null' : Array.isArray(actual) ? 'array' : typeof actual) === expected
      );
    case 'contains':
      return typeof actual === 'string' && typeof expected === 'string'
        ? actual.includes(expected)
        : Array.isArray(actual) && actual.some((v) => isDeepStrictEqual(v, expected));
    default:
      if (
        typeof actual !== 'number' ||
        typeof expected !== 'number' ||
        !Number.isFinite(actual) ||
        !Number.isFinite(expected)
      )
        throw new Error('Porównanie wymaga skończonych liczb.');
      if (predicate === 'greater') return actual > expected;
      if (predicate === 'less') return actual < expected;
      return Math.abs(actual - expected) <= tolerance;
  }
}

/** Execute against a frozen copy. Only redacted copies leave the runner. */
export async function runScenario(
  input: RunRequest,
  options: { signal?: AbortSignal; onUpdate?: (run: RunResult) => void } = {},
): Promise<RunResult> {
  validateRunRequest(input);
  const request: RunRequest = structuredClone(input);
  const { project } = request;
  const scenario = project.scenarios.find((s) => s.id === request.scenarioId)!;
  const env: Values = {
    ...project.environment.variables,
    ...request.secrets,
    baseUrl: project.environment.baseUrl,
  };
  const secretValues = new Set<string>(Object.values(request.secrets ?? {}).filter(Boolean));
  let executedSteps = 0;
  let reportBytes = 0;
  for (const name of project.environment.secretNames)
    if (typeof env[name] === 'string' && env[name]) secretValues.add(env[name] as string);
  function collect(value: unknown) {
    if (!value || typeof value !== 'object') return;
    for (const [key, val] of Object.entries(value)) {
      if (sensitive.test(key) && typeof val === 'string' && val) {
        secretValues.add(val);
        if (/^Bearer /i.test(val)) secretValues.add(val.slice(7));
      }
      if (/^(url|baseUrl)$/i.test(key) && typeof val === 'string') {
        try {
          const url = new URL(val, 'http://placeholder.invalid');
          if (url.password) {
            secretValues.add(url.password);
            secretValues.add(decodeURIComponent(url.password));
          }
          for (const [name, item] of url.searchParams)
            if (sensitive.test(name) && item) secretValues.add(item);
        } catch {
          /* Invalid URLs are diagnosed by the request operation. */
        }
      }
      collect(val);
    }
  }
  collect(project);
  function redact<T>(value: T): T {
    const replace = (v: unknown): unknown => {
      if (typeof v === 'string') {
        for (const secret of [...secretValues].sort((a, b) => b.length - a.length)) {
          const forms = [
            secret,
            encodeURIComponent(secret),
            new URLSearchParams({ v: secret }).toString().slice(2),
          ];
          for (const form of forms) v = (v as string).split(form).join('[REDACTED]');
        }
        return v;
      }
      if (Array.isArray(v)) return v.map(replace);
      if (v && typeof v === 'object')
        return Object.fromEntries(
          Object.entries(v).map(([key, val]) => [
            key,
            sensitive.test(key) ? '[REDACTED]' : replace(val),
          ]),
        );
      return v;
    };
    return replace(value) as T;
  }
  const run: RunResult = {
    id: randomUUID(),
    scenarioId: scenario.id,
    mode: request.mode,
    startedAt: new Date().toISOString(),
    status: 'running',
    steps: [],
    diagnostic: !!(request.singleNodeId || request.targetNodeId),
    snapshot: { graph: scenario.graph, cleanup: scenario.cleanup },
  };
  function notify() {
    options.onUpdate?.(redact(run));
  }
  const resolve = (b: Binding, context: Context): Json | undefined => {
    if (b.kind === 'expression') {
      try {
        return evaluateOperator(
          b.operator,
          Object.fromEntries(
            Object.entries(b.inputs).map(([name, child]) => [name, resolve(child, context)]),
          ),
          getPath,
        );
      } catch (error) {
        throw new Error(
          `${b.operator}: ${error instanceof Error ? error.message : 'Błąd operatora.'}`,
        );
      }
    }
    if (b.kind === 'literal') return b.value;
    if (b.kind === 'parameter')
      return own(context.parameters, b.name) ? context.parameters[b.name] : undefined;
    if (b.kind === 'environment') return own(env, b.name) ? env[b.name] : undefined;
    const source = context.outputs.get(b.nodeId);
    return source ? (getPath(source.outputs[b.port], b.path) as Json | undefined) : undefined;
  };
  function template(text: string, values: Values, encode = false): string {
    return text.replace(/\{\{\s*([^{}]+?)\s*\}\}/g, (_, key: string) => {
      const value = own(values, key) ? values[key] : env[key];
      if (value === undefined) throw new Error(`Brak argumentu szablonu: ${key}.`);
      const result = typeof value === 'object' ? JSON.stringify(value) : String(value);
      return encode ? encodeURIComponent(result) : result;
    });
  }
  function bodyTemplate(value: Json, values: Values): Json {
    if (typeof value === 'string') {
      const match = value.match(/^\{\{\s*([^{}]+?)\s*\}\}$/);
      if (match) {
        const v = own(values, match[1]) ? values[match[1]] : env[match[1]];
        if (v === undefined) throw new Error(`Brak argumentu body: ${match[1]}.`);
        return v;
      }
      return template(value, values);
    }
    if (Array.isArray(value)) return value.map((v) => bodyTemplate(v, values));
    if (value && typeof value === 'object')
      return Object.fromEntries(
        Object.entries(value).map(([k, v]) => [k, bodyTemplate(v, values)]),
      );
    return value;
  }
  async function http(node: TestNode, values: Values, step: StepResult, context: Context) {
    let address = template(node.config.url ?? '', values);
    address = address.replace(/\/:([A-Za-z_][\w-]*)/g, (_, key: string) => {
      if (values[key] === undefined) throw new Error(`Brak argumentu ścieżki: ${key}.`);
      return `/${encodeURIComponent(String(values[key]))}`;
    });
    // A configured base URL is an API prefix, including its pathname.
    const url = new URL(
      /^https?:\/\//i.test(address)
        ? address
        : `${project.environment.baseUrl.replace(/\/+$/, '')}/${address.replace(/^\/+/, '')}`,
    );
    if (!['http:', 'https:'].includes(url.protocol) || url.username || url.password)
      throw new Error('Dozwolone są adresy HTTP/HTTPS bez danych logowania w URL.');
    for (const [key, val] of Object.entries(node.config.query ?? {}))
      url.searchParams.set(key, template(val, values));
    for (const [key, val] of url.searchParams)
      if (sensitive.test(key) && val) secretValues.add(val);
    const headers = Object.fromEntries(
      Object.entries(node.config.headers ?? {}).map(([key, val]) => [key, template(val, values)]),
    );
    const body = values.body === undefined ? undefined : bodyTemplate(values.body, values);
    if (body !== undefined && !Object.keys(headers).some((k) => k.toLowerCase() === 'content-type'))
      headers['Content-Type'] = 'application/json';
    collect(headers);
    collect(body);
    const method = node.config.method ?? 'GET';
    if (body !== undefined && ['GET', 'HEAD'].includes(method))
      throw new Error('GET i HEAD nie przyjmują body.');
    step.request = {
      method,
      url: url.toString(),
      headers,
      ...(body !== undefined ? { body } : {}),
    };
    if (request.mode === 'simulation') {
      if (node.config.sampleResponse === undefined)
        throw new Error('Brak przykładowej odpowiedzi do symulacji.');
      step.outputs = {
        body: node.config.sampleResponse,
        status: node.config.sampleStatus ?? 200,
        headers: {},
        duration: 0,
      };
      collect(step.outputs);
      return;
    }
    const timeout = AbortSignal.timeout(node.config.timeoutMs ?? 10000);
    const signal = context.signal ? AbortSignal.any([context.signal, timeout]) : timeout;
    const start = performance.now();
    try {
      const response = await fetch(url, {
        method,
        headers,
        body: body === undefined ? undefined : JSON.stringify(body),
        signal,
        redirect: 'error',
      });
      const responseHeaders = Object.fromEntries(response.headers.entries());
      collect(responseHeaders);
      if (Number(response.headers.get('content-length')) > 2 * 1024 * 1024) {
        await response.body?.cancel();
        throw new Error('Odpowiedź przekracza limit 2 MiB.');
      }
      const reader = response.body?.getReader();
      let bytes = 0;
      const chunks: Uint8Array[] = [];
      if (reader)
        while (true) {
          const next = await reader.read();
          if (next.done) break;
          bytes += next.value.length;
          if (bytes > 2 * 1024 * 1024) {
            await reader.cancel();
            throw new Error('Odpowiedź przekracza limit 2 MiB.');
          }
          chunks.push(next.value);
        }
      const text = Buffer.concat(chunks).toString('utf8');
      let result: Json = text;
      try {
        result = JSON.parse(text) as Json;
      } catch {
        /* Non-JSON responses remain text. */
      }
      const queue: { value: Json; depth: number }[] = [{ value: result, depth: 0 }];
      while (queue.length) {
        const { value, depth } = queue.pop()!;
        if (depth > 100)
          throw new Error('Odpowiedź JSON przekracza limit 100 poziomów zagnieżdżenia.');
        if (typeof value === 'number' && !Number.isFinite(value))
          throw new Error('Odpowiedź JSON zawiera liczbę poza skończonym zakresem.');
        if (value && typeof value === 'object')
          for (const child of Object.values(value)) queue.push({ value: child, depth: depth + 1 });
      }
      step.outputs = {
        body: result,
        status: response.status,
        headers: responseHeaders,
        duration: Math.round(performance.now() - start),
      };
      collect(step.outputs);
    } catch (error) {
      if (timeout.aborted && !context.signal?.aborted)
        throw new Error(`Przekroczono czas requestu (${node.config.timeoutMs ?? 10000} ms).`);
      throw error;
    }
  }
  async function execute(node: TestNode, context: Context): Promise<StepResult> {
    const start = performance.now();
    const step: StepResult = {
      nodeId: node.id,
      name: node.name,
      kind: node.kind,
      status: 'passed',
      phase: context.phase,
      inputs: {},
      outputs: {},
      durationMs: 0,
    };
    try {
      if (++executedSteps > 1000)
        throw new Error('Przekroczono limit 1000 kroków (łącznie z komponentami).');
      const values: Values = Object.fromEntries(
        Object.entries(node.inputs).map(([key, b]) => [key, resolve(b, context)]),
      );
      for (const port of inputPorts(node, project))
        if (values[port.name] === undefined && port.defaultValue !== undefined)
          values[port.name] = port.defaultValue;
      step.inputs = compact(values);
      // Assertions can inspect missing fields explicitly. Other operations require their declared inputs.
      if (!['assert', 'condition', 'if', 'select'].includes(node.kind))
        for (const port of inputPorts(node, project)) {
          const value = values[port.name];
          if (port.required && value === undefined)
            throw new Error(`Brak wymaganego wejścia: ${port.name}.`);
          if (
            value !== undefined &&
            port.type !== 'any' &&
            (port.type === 'array'
              ? !Array.isArray(value)
              : port.type === 'object'
                ? value === null || typeof value !== 'object' || Array.isArray(value)
                : typeof value !== port.type)
          )
            throw new Error(`Wejście ${port.name}: wymagany typ ${port.type}.`);
        }
      if (node.kind === 'variable') step.outputs = compact({ value: values.value });
      else if (node.kind === 'request') await http(node, values, step, context);
      else if (node.kind === 'select')
        step.outputs = compact({
          value: getPath(values.value, template(node.config.path ?? '', values)) as
            Json | undefined,
        });
      else if (node.kind === 'build') {
        if (Object.values(values).some((v) => v === undefined))
          throw new Error('Brak wartości do zbudowania danych.');
        step.outputs.value =
          node.config.buildType === 'array' ? Object.values(compact(values)) : compact(values);
      } else if (node.kind === 'formula') {
        step.outputs = compact({
          value: evaluateOperator(node.config.operation ?? 'multiply', values, getPath),
        });
      } else if (node.kind === 'assert' || node.kind === 'condition' || node.kind === 'if') {
        const passed =
          node.config.predicates?.length && node.kind === 'condition'
            ? (() => {
                const checks = node.config.predicates!.map((p) =>
                  check(
                    p.predicate,
                    getPath(values[p.input], template(p.path ?? '', values)) as Json | undefined,
                    values[p.expected],
                    node.config.tolerance,
                  ),
                );
                return node.config.match === 'any' ? checks.some(Boolean) : checks.every(Boolean);
              })()
            : check(
                node.config.predicate ?? 'equals',
                values.actual,
                values.expected,
                node.config.tolerance,
              );
        step.outputs = compact({ value: values.actual, passed });
        if (values.actual !== undefined) step.actual = values.actual;
        if (values.expected !== undefined) step.expected = values.expected;
        if (node.kind === 'if')
          step.message = passed ? 'Wybrano gałąź: tak.' : 'Wybrano gałąź: nie.';
        if (!passed && node.kind !== 'if') {
          step.status = node.kind === 'condition' ? 'blocked' : 'failed';
          step.message =
            node.kind === 'condition'
              ? 'Niewłaściwe dane: warunek wejściowy niespełniony.'
              : 'Porównanie nie powiodło się.';
        }
      } else if (node.kind === 'component') {
        const component = definition(project, node)!;
        const key = `${component.id}@${component.revision}`;
        if (context.stack.includes(key) || context.stack.length >= 16)
          throw new Error('Rekurencja lub zbyt głębokie zagnieżdżenie komponentów.');
        const nested: Context = {
          ...context,
          parameters: values,
          outputs: new Map(),
          stack: [...context.stack, key],
        };
        step.children = await executeGraph(component.graph, nested, false);
        for (const port of component.outputs) {
          const value = resolve(port.binding, nested);
          if (value !== undefined) step.outputs[port.name] = value;
        }
        if (step.children.some((s) => s.status === 'cancelled')) step.status = 'cancelled';
        else if (step.children.some((s) => s.status === 'error')) step.status = 'error';
        else if (step.children.some((s) => s.status === 'blocked')) step.status = 'blocked';
        else if (step.children.some((s) => s.status === 'failed')) step.status = 'failed';
        if (step.status !== 'passed')
          step.message = 'Sprawdź wynik wewnętrznych kroków komponentu.';
      }
    } catch (error) {
      step.status = context.signal?.aborted ? 'cancelled' : 'error';
      step.message = error instanceof Error ? error.message : 'Błąd wykonania.';
    }
    step.durationMs = Math.round(performance.now() - start);
    const size = Buffer.byteLength(JSON.stringify(step));
    if (reportBytes + size > 16 * 1024 * 1024) {
      step.status = 'error';
      step.message = 'Przekroczono limit danych przebiegu 16 MiB.';
      step.inputs = {};
      step.outputs = {};
      delete step.children;
      delete step.request;
      delete step.expected;
      delete step.actual;
    } else reportBytes += size;
    return step;
  }
  async function executeGraph(
    graph: Graph,
    context: Context,
    publish: boolean,
    selected?: Set<string>,
  ): Promise<StepResult[]> {
    // Every operation is awaited. Control edges activate a path; data references
    // additionally require finished producers. Older graphs without Start keep their semantics.
    const results: StepResult[] = [];
    const pending = graph.nodes.filter((n) => !selected || selected.has(n.id));
    const graphIds = new Set(pending.map((n) => n.id));
    const plan = planExecution(graph);
    let stopped = false;
    while (pending.length) {
      const index = pending.findIndex((n) =>
        [...plan.dependencies.get(n.id)!].every(
          (id) => !graphIds.has(id) || context.outputs.has(id),
        ),
      );
      if (index < 0) throw new Error('Graf zawiera nierozwiązywalne zależności.');
      const node = pending.splice(index, 1)[0];
      const deps = [...plan.dependencies.get(node.id)!];
      const incoming = graph.edges.filter((e) => e.target === node.id);
      const activeOrder = incoming.filter((edge) => {
        const source = context.outputs.get(edge.source);
        return (
          source &&
          !['skipped', 'blocked', 'error', 'cancelled'].includes(source.status) &&
          (edge.branch === undefined || source.outputs.passed === (edge.branch === 'true'))
        );
      });
      // Alternative branches merge with OR. An inactive branch is never a failure.
      const inactive =
        !(request.singleNodeId === node.id && selected?.size === 1) &&
        ((plan.reachable && !plan.reachable.has(node.id)) ||
          (incoming.length > 0 &&
            activeOrder.length === 0 &&
            incoming.every((edge) => {
              const source = context.outputs.get(edge.source);
              return (
                source?.status === 'skipped' ||
                (edge.branch !== undefined &&
                  source?.status === 'passed' &&
                  source.outputs.passed !== (edge.branch === 'true'))
              );
            })) ||
          (!incoming.length && deps.some((id) => context.outputs.get(id)?.status === 'skipped')));
      const bad = deps.some((id) => {
        const source = context.outputs.get(id);
        if (!source) return true;
        const dataDependency = Object.values(node.inputs)
          .flatMap(references)
          .some((b) => b.nodeId === id);
        if (!dataDependency && activeOrder.length && source.status === 'skipped') return false;
        if (context.phase === 'cleanup') {
          // A component may create a resource and fail later. Exported resource IDs
          // remain usable for cleanup, while main-phase business steps stay blocked.
          const refs = Object.values(node.inputs)
            .flatMap(references)
            .filter((b) => b.nodeId === id);
          if (refs.length && refs.every((b) => resolve(b, context) !== undefined)) return false;
          // An order-only cleanup dependency means "after completion", even on failure.
          if (!refs.length) return false;
        }
        return ['error', 'blocked', 'skipped', 'cancelled'].includes(source.status);
      });
      const missingCleanupInput =
        context.phase === 'cleanup' &&
        Object.values(node.inputs)
          .flatMap(references)
          .some((b) => resolve(b, context) === undefined);
      let step: StepResult;
      const status = context.signal?.aborted
        ? 'cancelled'
        : inactive
          ? 'skipped'
          : stopped
            ? 'skipped'
            : missingCleanupInput
              ? 'skipped'
              : bad
                ? context.phase === 'cleanup'
                  ? 'skipped'
                  : 'blocked'
                : undefined;
      if (status)
        step = {
          nodeId: node.id,
          name: node.name,
          kind: node.kind,
          status,
          phase: context.phase,
          inputs: {},
          outputs: {},
          durationMs: 0,
          message: inactive
            ? 'Krok poza aktywną ścieżką wykonania.'
            : status === 'cancelled'
              ? 'Anulowano.'
              : bad || missingCleanupInput
                ? 'Niedostępne dane lub niespełniony warunek wcześniejszego kroku.'
                : 'Zatrzymano po błędzie.',
        };
      else step = await execute(node, context);
      results.push(step);
      context.outputs.set(node.id, step);
      if (
        scenario.errorPolicy === 'stop' &&
        context.phase === 'main' &&
        ['failed', 'error', 'blocked'].includes(step.status)
      )
        stopped = true;
      if (publish) {
        run.steps.push(step);
        notify();
      }
    }
    return results;
  }
  const budget = AbortSignal.timeout(120000);
  const context: Context = {
    parameters: scenario.parameters,
    outputs: new Map(),
    signal: options.signal ? AbortSignal.any([options.signal, budget]) : budget,
    stack: [],
    phase: 'main',
  };
  let selection: Set<string> | undefined;
  if (request.singleNodeId) {
    selection = new Set([request.singleNodeId]);
    for (const step of request.previousRun?.steps ?? [])
      if (step.phase === 'main' && step.nodeId !== request.singleNodeId)
        context.outputs.set(step.nodeId, step);
    run.error = request.previousRun
      ? `Wejścia historyczne z przebiegu ${request.previousRun.id}, ${request.previousRun.finishedAt}.`
      : 'Pojedynczy krok: brak historycznych wejść.';
  } else if (request.targetNodeId) {
    selection = new Set<string>();
    const visit = (id: string) => {
      if (selection!.has(id)) return;
      selection!.add(id);
      dependencies(scenario.graph, id).forEach(visit);
    };
    visit(request.targetNodeId);
  }
  notify();
  try {
    await executeGraph(scenario.graph, context, true, selection);
  } catch (error) {
    run.error = error instanceof Error ? error.message : 'Błąd wykonania grafu.';
  } finally {
    // Cleanup gets its own budget, unaffected by cancellation of the main test.
    try {
      await executeGraph(
        scenario.cleanup,
        { ...context, phase: 'cleanup', signal: AbortSignal.timeout(10000) },
        true,
      );
    } catch (error) {
      run.error = [
        run.error,
        `Błąd sprzątania: ${error instanceof Error ? error.message : 'nieznany'}`,
      ]
        .filter(Boolean)
        .join(' ');
    }
  }
  if (budget.aborted) run.error = 'Przekroczono limit czasu przebiegu 120 s.';
  run.status = options.signal?.aborted
    ? 'cancelled'
    : run.steps.some((s) => ['failed', 'error', 'blocked', 'cancelled'].includes(s.status)) ||
        (!request.singleNodeId && run.error)
      ? 'failed'
      : 'passed';
  run.finishedAt = new Date().toISOString();
  notify();
  return redact(run);
}

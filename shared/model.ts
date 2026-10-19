import type { Operator } from './operators';
export type Json = null | boolean | number | string | Json[] | { [key: string]: Json };
export type ValueType = 'any' | 'string' | 'number' | 'boolean' | 'object' | 'array';
export type Binding =
  | { kind: 'expression'; operator: Operator; inputs: Record<string, Binding> }
  | { kind: 'literal'; value: Json }
  | { kind: 'reference'; nodeId: string; port: string; path: string }
  | { kind: 'parameter'; name: string }
  | { kind: 'environment'; name: string };
export interface Port {
  name: string;
  label?: string;
  type: ValueType;
  contractId?: string;
  required?: boolean;
  defaultValue?: Json;
  exposed?: boolean;
}
export type NodeKind =
  | 'request'
  | 'select'
  | 'build'
  | 'formula'
  | 'assert'
  | 'condition'
  | 'component'
  | 'variable'
  | 'trigger'
  | 'if';
export type Predicate =
  | 'equals'
  | 'notEquals'
  | 'greater'
  | 'less'
  | 'exists'
  | 'notEmpty'
  | 'contains'
  | 'type'
  | 'approximately';
export interface NodeConfig {
  method?: string;
  url?: string;
  headers?: Record<string, string>;
  query?: Record<string, string>;
  timeoutMs?: number;
  sampleResponse?: Json;
  sampleStatus?: number;
  contractId?: string;
  path?: string;
  operation?: Operator;
  predicate?: Predicate;
  tolerance?: number;
  buildType?: 'object' | 'array';
  componentId?: string;
  revision?: number;
  inputPorts?: Port[];
  templateInputs?: Port[];
  predicates?: { input: string; expected: string; predicate: Predicate; path?: string }[];
  match?: 'all' | 'any';
}
export interface TestNode {
  id: string;
  kind: NodeKind;
  name: string;
  position: { x: number; y: number };
  inputs: Record<string, Binding>;
  config: NodeConfig;
}
// Data edges are derived from reference bindings. Only sequencing edges are stored here.
export interface OrderEdge {
  branch?: 'true' | 'false';
  id: string;
  source: string;
  target: string;
}
export interface Graph {
  nodes: TestNode[];
  edges: OrderEdge[];
}
export interface ComponentDefinition {
  id: string;
  revision: number;
  name: string;
  description: string;
  inputs: Port[];
  outputs: (Port & { binding: Binding })[];
  graph: Graph;
  action?: { input: string; type: ValueType; contractId?: string };
}
export interface Contract {
  id: string;
  name: string;
  sample: Json;
}
export interface Environment {
  baseUrl: string;
  variables: Record<string, string>;
  secretNames: string[];
}
export interface Scenario {
  id: string;
  name: string;
  description: string;
  graph: Graph;
  cleanup: Graph;
  parameters: Record<string, Json>;
  errorPolicy: 'stop' | 'continue';
}
export interface Project {
  version: 1;
  id: string;
  name: string;
  scenarios: Scenario[];
  components: ComponentDefinition[];
  contracts: Contract[];
  environment: Environment;
}
export type StepStatus = 'passed' | 'failed' | 'error' | 'blocked' | 'skipped' | 'cancelled';
export interface StepResult {
  nodeId: string;
  name: string;
  kind: NodeKind;
  status: StepStatus;
  phase: 'main' | 'cleanup';
  inputs: Record<string, Json>;
  outputs: Record<string, Json>;
  durationMs: number;
  message?: string;
  expected?: Json;
  actual?: Json;
  request?: { method: string; url: string; headers: Record<string, string>; body?: Json };
  children?: StepResult[];
}
export interface RunRequest {
  project: Project;
  scenarioId: string;
  mode: 'simulation' | 'live';
  targetNodeId?: string;
  singleNodeId?: string;
  previousRun?: RunResult;
  secrets?: Record<string, string>;
}
export interface RunResult {
  id: string;
  scenarioId: string;
  mode: 'simulation' | 'live';
  startedAt: string;
  finishedAt?: string;
  status: 'running' | 'passed' | 'failed' | 'cancelled';
  steps: StepResult[];
  diagnostic?: boolean;
  snapshot?: { graph: Graph; cleanup: Graph };
  error?: string;
}
export const literal = (value: Json): Extract<Binding, { kind: 'literal' }> => ({
  kind: 'literal',
  value,
});
export const reference = (
  nodeId: string,
  port = 'body',
  path = '',
): Extract<Binding, { kind: 'reference' }> => ({
  kind: 'reference',
  nodeId,
  port,
  path,
});
export const uid = (prefix = 'node') => `${prefix}-${globalThis.crypto.randomUUID().slice(0, 8)}`;

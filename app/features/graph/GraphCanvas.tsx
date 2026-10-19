import { readVariable, VARIABLE_MIME } from '../../../shared/variables';
import { bindInput, hasCycle } from '../../../shared/graph';
import { operators, isOperator, type Operator } from '../../../shared/operators';
import { AssertionTypeSelect } from '../inspector/fields';
import { useCallback, useMemo, useRef, useState } from 'react';
import {
  Background,
  BackgroundVariant,
  Controls,
  Handle,
  MarkerType,
  MiniMap,
  Panel,
  Position,
  ReactFlow,
  ReactFlowProvider,
  SelectionMode,
  useReactFlow,
} from '@xyflow/react';
import type { Connection, Edge, Node, NodeChange, NodeProps } from '@xyflow/react';
import {
  IconArrowsSplit,
  IconCircle,
  IconBraces,
  IconCheck,
  IconCopy,
  IconClipboard,
  IconLayersSubtract,
  IconCube,
  IconExternalLink,
  IconFilter,
  IconGitBranch,
  IconMathFunction,
  IconPlus,
  IconTrash,
  IconWorld,
  IconX,
} from '@tabler/icons-react';
import {
  compatible,
  deleteNodes,
  inputPorts,
  kindDescriptions,
  kindLabels,
  outputPorts,
  visualEdges,
} from '../../../shared/graph';
import { literal } from '../../../shared/model';
import type {
  Binding,
  ComponentDefinition,
  Graph,
  Json,
  NodeKind,
  Port,
  Project,
  StepResult,
  TestNode,
} from '../../../shared/model';
import { connectGraph, removeConnections } from './operations';
import '@xyflow/react/dist/style.css';
import './graph.css';

export interface GraphCanvasProps {
  graph: Graph;
  project: Project;
  selectedIds: string[];
  onCopy: () => void;
  onPaste: () => void;
  onDuplicate: () => void;
  canPaste: boolean;
  onSelect: (ids: string[]) => void;
  onChange: (graph: Graph) => void;
  onAdd: (
    kind: NodeKind,
    position: { x: number; y: number },
    component?: ComponentDefinition,
    source?: { nodeId: string; port: string },
  ) => void;
  onAddOperator?: (operator: Operator, position: { x: number; y: number }) => void;
  onSaveSelection: () => void;
  onEditComponent: (node: TestNode) => void;
  steps?: StepResult[];
}
const icons = {
  trigger: IconCircle,
  if: IconGitBranch,
  variable: IconBraces,
  request: IconWorld,
  select: IconFilter,
  build: IconBraces,
  formula: IconMathFunction,
  assert: IconCheck,
  condition: IconGitBranch,
  component: IconCube,
};
const statusLabels = {
  passed: 'Zaliczony',
  failed: 'Niezgodność',
  error: 'Błąd',
  blocked: 'Zablokowany',
  skipped: 'Pominięty',
  cancelled: 'Anulowany',
};
const MIME = 'application/apiforge-node';
type Source = { nodeId: string; port: string };
type BlockData = {
  node: TestNode;
  graph: Graph;
  project: Project;
  step?: StepResult;
  update: (name: string, value: Json) => void;
  bind: (name: string, binding: Binding) => void;
  action: (event: React.MouseEvent, node: TestNode, port?: string) => void;
  edit: () => void;
};
type FlowNode = Node<BlockData, 'block'>;

function bindingLabel(binding: Binding, graph: Graph) {
  if (binding.kind === 'expression') return `${operators[binding.operator].label} · blok`;
  if (binding.kind === 'reference')
    return `${graph.nodes.find((n) => n.id === binding.nodeId)?.name ?? 'Krok'} → ${binding.port}${binding.path ? `.${binding.path}` : ''}`;
  if (binding.kind === 'parameter') return `Argument → ${binding.name}`;
  if (binding.kind === 'environment') return `Środowisko → ${binding.name}`;
  return '';
}
function InlineValue({
  port,
  value,
  onCommit,
}: {
  port: Port;
  value: Json | undefined;
  onCommit: (value: Json) => void;
}) {
  const format = (v: Json | undefined) =>
    v === undefined ? '' : typeof v === 'string' ? v : JSON.stringify(v);
  const [draft, setDraft] = useState(format(value));
  const [previous, setPrevious] = useState(value);
  const [invalid, setInvalid] = useState(false);
  if (value !== previous) {
    setPrevious(value);
    setDraft(format(value));
  }
  const commit = () => {
    if (draft === format(value)) return;
    let parsed: Json = draft;
    if (port.type === 'any') {
      try {
        parsed = JSON.parse(draft);
      } catch {
        /* Plain text remains a string. */
      }
    }
    if (port.type === 'number') {
      if (!draft.trim() || !Number.isFinite(Number(draft))) {
        setInvalid(true);
        return;
      }
      parsed = Number(draft);
    }
    if (['object', 'array', 'boolean'].includes(port.type)) {
      try {
        parsed = JSON.parse(draft);
        if (
          port.type === 'array'
            ? !Array.isArray(parsed)
            : port.type === 'object'
              ? !parsed || Array.isArray(parsed) || typeof parsed !== 'object'
              : typeof parsed !== 'boolean'
        )
          throw Error();
      } catch {
        setInvalid(true);
        return;
      }
    }
    setInvalid(false);
    onCommit(parsed);
  };
  return (
    <input
      aria-label={port.label ?? port.name}
      aria-invalid={invalid}
      className={`af-inline-input nodrag nowheel ${invalid ? 'is-invalid' : ''}`}
      value={draft}
      placeholder={port.required ? 'Wymagane' : 'Opcjonalne'}
      onChange={(e) => {
        setDraft(e.target.value);
        setInvalid(false);
      }}
      onBlur={commit}
      onKeyDown={(e) => {
        e.stopPropagation();
        if (e.key === 'Enter') e.currentTarget.blur();
      }}
      title={invalid ? `Wpisz poprawną wartość typu ${port.type}` : undefined}
    />
  );
}
function BlockNode({ data, selected }: NodeProps<FlowNode>) {
  const { node, project, graph, step } = data,
    Icon = icons[node.kind];
  const inputs = inputPorts(node, project),
    outputs = outputPorts(node, project);
  return (
    <div
      className={`af-block af-kind-${node.kind} ${selected ? 'is-selected' : ''} ${step ? `af-step-${step.status}` : ''}`}
    >
      {node.kind !== 'trigger' && (
        <Handle
          type="target"
          position={Position.Top}
          id="__order"
          className="af-order-handle"
          title="Kolejność: po wykonaniu"
        />
      )}
      <div className="af-block-head">
        <span className="af-block-icon">
          <Icon size={18} stroke={1.8} />
        </span>
        <div>
          <small>
            {kindLabels[node.kind]}
            {node.kind === 'component' && ` · r${node.config.revision}`}
          </small>
          <strong title={node.name}>{node.name}</strong>
        </div>
        {node.kind === 'component' && (
          <button
            className="af-icon-button nodrag"
            onClick={data.edit}
            title="Otwórz wnętrze komponentu"
          >
            <IconExternalLink size={16} />
          </button>
        )}
      </div>
      {node.kind === 'request' && (
        <div className="af-request-line">
          <b>{node.config.method ?? 'GET'}</b>
          <code title={node.config.url}>{node.config.url || '/endpoint'}</code>
        </div>
      )}
      {node.kind === 'select' && (
        <div className="af-node-subtitle">
          <code>{node.config.path || 'Cała wartość'}</code>
        </div>
      )}
      {node.kind === 'formula' && (
        <div className="af-node-subtitle">
          {operators[node.config.operation ?? 'multiply'].label}
        </div>
      )}
      <div className="af-node-ports">
        {inputs.map((port) => {
          const binding = node.inputs[port.name];
          const field =
            port.exposed ||
            (node.kind !== 'component' && port.name !== 'body' && inputs.length <= 3);
          return (
            <div
              className="af-input-port"
              key={port.name}
              onDragOver={(event) => {
                if (event.dataTransfer.types.includes(VARIABLE_MIME)) {
                  event.preventDefault();
                  event.stopPropagation();
                }
              }}
              onDropCapture={(event) => {
                const variable = readVariable(event.dataTransfer);
                if (variable) {
                  event.preventDefault();
                  event.stopPropagation();
                  data.bind(port.name, variable.binding);
                }
              }}
            >
              <Handle
                type="target"
                position={Position.Left}
                id={`in:${port.name}`}
                title={`Wejście ${port.label ?? port.name} (${port.type})`}
              />
              <label>
                <span>
                  {port.label ?? port.name}
                  {port.required && <i> *</i>}
                </span>
                {binding && binding.kind !== 'literal' ? (
                  <span className="af-binding" title={bindingLabel(binding, graph)}>
                    {bindingLabel(binding, graph)}
                  </span>
                ) : (node.kind === 'assert' || node.kind === 'condition' || node.kind === 'if') &&
                  node.config.predicate === 'type' &&
                  port.name === 'expected' ? (
                  <AssertionTypeSelect
                    className="af-inline-input nodrag nowheel"
                    value={binding?.kind === 'literal' ? String(binding.value) : ''}
                    onChange={(value) => data.update(port.name, value)}
                  />
                ) : field ? (
                  <InlineValue
                    port={port}
                    value={binding?.kind === 'literal' ? binding.value : undefined}
                    onCommit={(v) => data.update(port.name, v)}
                  />
                ) : (
                  <small className={!binding && port.required ? 'af-missing' : ''}>
                    {!binding ? (port.required ? 'Brak wartości' : 'opcjonalne') : port.type}
                  </small>
                )}
              </label>
            </div>
          );
        })}
      </div>
      <div className="af-output-ports">
        {outputs.map((port) => (
          <div className="af-output-port" key={port.name}>
            <span>{port.label ?? port.name}</span>
            <small>
              {project.contracts.find((c) => c.id === port.contractId)?.name ?? port.type}
            </small>
            <Handle
              type="source"
              position={Position.Right}
              id={`out:${port.name}`}
              title={`Wyjście ${port.name}`}
            />
          </div>
        ))}
      </div>
      <div className="af-node-footer">
        {step ? (
          <span className={`af-status af-status-${step.status}`} title={step.message}>
            {statusLabels[step.status]} <small>{step.durationMs} ms</small>
          </span>
        ) : (
          <span>Gotowy do konfiguracji</span>
        )}
        <button
          className="af-add-action nodrag"
          onClick={(event) => data.action(event, node)}
          title="Dodaj akcję"
        >
          <IconPlus size={14} /> Akcja
        </button>
      </div>
      <div className="af-control-outputs">
        {(node.kind === 'if' ? ['true', 'false'] : ['order']).map((port) => (
          <div className="af-control-output" key={port}>
            <button
              className="nodrag"
              title={`Dodaj następny krok: ${port === 'true' ? 'tak' : port === 'false' ? 'nie' : 'dalej'}`}
              onClick={(event) => data.action(event, node, `__${port}`)}
            >
              {port === 'true' ? 'Tak →' : port === 'false' ? 'Nie →' : 'Dalej →'}
            </button>
            <Handle
              type="source"
              position={Position.Bottom}
              id={`__${port}`}
              className="af-order-handle"
              title={`Kolejność: ${port === 'true' ? 'tak' : port === 'false' ? 'nie' : 'następny krok'}`}
            />
          </div>
        ))}
      </div>
    </div>
  );
}
const nodeTypes = { block: BlockNode };
interface ActionMenu {
  left: number;
  top: number;
  position: { x: number; y: number };
  source?: Source;
}

function Canvas(props: GraphCanvasProps) {
  const {
    graph,
    project,
    selectedIds,
    onChange,
    onSelect,
    onCopy,
    onPaste,
    onDuplicate,
    canPaste,
    onAdd,
    onSaveSelection,
    onEditComponent,
    steps,
  } = props;
  const wrapper = useRef<HTMLDivElement>(null);
  const { screenToFlowPosition, fitView } = useReactFlow<FlowNode>();
  const [menu, setMenu] = useState<ActionMenu | null>(null),
    [search, setSearch] = useState(''),
    [error, setError] = useState('');
  const [selectedEdges, setSelectedEdges] = useState<string[]>([]);
  const [measurements, setMeasurements] = useState<
    Record<string, { width: number; height: number }>
  >({});
  const openMenu = useCallback(
    (clientX: number, clientY: number, source?: Source) => {
      const rect = wrapper.current!.getBoundingClientRect();
      setMenu({
        left: Math.max(12, Math.min(clientX - rect.left, rect.width - 310)),
        top: Math.max(12, Math.min(clientY - rect.top, rect.height - 410)),
        position: screenToFlowPosition({ x: clientX, y: clientY }),
        source,
      });
      setSearch('');
      setError('');
    },
    [screenToFlowPosition],
  );
  const nodeAction = useCallback(
    (event: React.MouseEvent, node: TestNode, port?: string) => {
      event.preventDefault();
      event.stopPropagation();
      openMenu(event.clientX, event.clientY, {
        nodeId: node.id,
        port:
          port ??
          (node.kind === 'trigger'
            ? '__order'
            : node.kind === 'if'
              ? '__true'
              : (outputPorts(node, project)[0]?.name ?? 'value')),
      });
    },
    [openMenu, project],
  );
  const nodes: FlowNode[] = useMemo(
    () =>
      graph.nodes.map((node) => ({
        id: node.id,
        type: 'block',
        position: node.position,
        measured: measurements[node.id],
        selected: selectedIds.includes(node.id),
        data: {
          node,
          graph,
          project,
          step: steps?.find((s) => s.nodeId === node.id),
          update: (name, value) =>
            onChange({
              ...graph,
              nodes: graph.nodes.map((n) =>
                n.id === node.id ? { ...n, inputs: { ...n.inputs, [name]: literal(value) } } : n,
              ),
            }),
          bind: (name, binding) => {
            const next = bindInput(graph, node.id, name, binding);
            if (hasCycle(next)) {
              setError('Ta zmienna utworzyłaby cykl zależności.');
              return;
            }
            onChange(next);
          },
          action: nodeAction,
          edit: () => onEditComponent(node),
        },
      })),
    [graph, project, selectedIds, steps, onChange, onEditComponent, nodeAction, measurements],
  );
  const edges: Edge[] = useMemo(
    () =>
      visualEdges(graph)
        .filter((e) => graph.nodes.some((n) => n.id === e.source))
        .map((e) => ({
          id: e.id,
          source: e.source,
          target: e.target,
          sourceHandle:
            e.kind === 'data' ? `out:${e.sourcePort}` : e.branch ? `__${e.branch}` : '__order',
          targetHandle: e.kind === 'data' ? `in:${e.targetInput}` : '__order',
          type: 'smoothstep',
          selected: selectedEdges.includes(e.id),
          markerEnd: {
            type: MarkerType.ArrowClosed,
            width: 16,
            height: 16,
            color: e.kind === 'data' ? '#b8b5b0' : '#a3a8ac',
          },
          style: {
            stroke: e.kind === 'data' ? '#b8b5b0' : '#a3a8ac',
            strokeWidth: 1.8,
            strokeDasharray: e.kind === 'order' ? '5 5' : undefined,
          },
          label:
            e.kind === 'order'
              ? e.branch === 'true'
                ? 'tak'
                : e.branch === 'false'
                  ? 'nie'
                  : 'dalej'
              : undefined,
          labelStyle: { fontSize: 10, fill: '#85817a' },
        })),
    [graph, selectedEdges],
  );
  const changeNodes = (changes: NodeChange<FlowNode>[]) => {
    setMeasurements((previous) => {
      let next = previous;
      for (const change of changes) {
        if (change.type !== 'dimensions' || !change.dimensions) continue;
        const size = change.dimensions,
          old = previous[change.id];
        if (old?.width !== size.width || old?.height !== size.height)
          next = { ...next, [change.id]: size };
      }
      return next;
    });
    let next = graph;
    let selected = [...selectedIds];
    let mutated = false;
    let selectionChanged = false;
    for (const c of changes) {
      if (c.type === 'remove') {
        next = deleteNodes(next, [c.id]);
        selected = selected.filter((id) => id !== c.id);
        mutated = true;
        selectionChanged = true;
      }
      if (c.type === 'position' && c.position) {
        next = {
          ...next,
          nodes: next.nodes.map((n) => (n.id === c.id ? { ...n, position: c.position! } : n)),
        };
        mutated = true;
      }
      if (c.type === 'select') {
        selected = c.selected
          ? [...new Set([...selected, c.id])]
          : selected.filter((id) => id !== c.id);
        selectionChanged = true;
      }
    }
    if (mutated) onChange(next);
    if (selectionChanged) onSelect(selected);
  };
  const connectionResult = (c: Connection | Edge) =>
    connectGraph(
      graph,
      project,
      c.source,
      c.sourceHandle?.replace(/^out:/, '') ?? '',
      c.target,
      c.targetHandle?.replace(/^in:/, '') ?? '',
    );
  const connect = (c: Connection) => {
    const result = connectionResult(c);
    if (result.graph) {
      onChange(result.graph);
      setError('');
    } else setError(result.error ?? 'Nieprawidłowe połączenie.');
  };
  const sourceNode = graph.nodes.find((n) => n.id === menu?.source?.nodeId);
  const sourcePort = sourceNode
    ? outputPorts(sourceNode, project).find((p) => p.name === menu?.source?.port)
    : undefined;
  const builtinKinds: NodeKind[] = menu?.source
    ? ['request', 'if', 'select', 'assert', 'condition', 'formula', 'build']
    : ['trigger', 'if', 'request', 'select', 'build', 'formula', 'assert', 'condition'];
  const actions = builtinKinds
    .filter(
      (kind) =>
        !sourcePort ||
        kind !== 'formula' ||
        compatible(sourcePort, { name: 'left', type: 'number' }),
    )
    .map((kind) => ({
      kind,
      name: kindLabels[kind],
      description: kindDescriptions[kind],
      component: undefined as ComponentDefinition | undefined,
    }));
  const latest = project.components.filter(
    (c) => !project.components.some((other) => other.id === c.id && other.revision > c.revision),
  );
  for (const component of latest) {
    if (
      menu?.source &&
      !menu.source.port.startsWith('__') &&
      (!component.action ||
        !sourcePort ||
        !compatible(sourcePort, {
          name: component.action.input,
          type: component.action.type,
          contractId: component.action.contractId,
        }))
    )
      continue;
    actions.push({
      kind: 'component',
      name: component.name,
      description: component.description || `Komponent · rewizja ${component.revision}`,
      component,
    });
  }
  function add(
    kind: NodeKind,
    component?: ComponentDefinition,
    position = menu!.position,
    source = menu?.source,
  ) {
    onAdd(kind, position, component, source);
    setMenu(null);
  }
  function removeSelected() {
    onChange(removeConnections(deleteNodes(graph, selectedIds), selectedEdges));
    onSelect([]);
    setSelectedEdges([]);
  }
  return (
    <div className="af-graph" ref={wrapper}>
      <ReactFlow<FlowNode>
        nodes={nodes}
        edges={edges}
        nodeTypes={nodeTypes}
        onNodesChange={changeNodes}
        onEdgesChange={(changes) => {
          const removed = changes.filter((c) => c.type === 'remove').map((c) => c.id);
          if (removed.length) onChange(removeConnections(graph, removed));
          setSelectedEdges((previous) => {
            let next = previous;
            for (const c of changes)
              if (c.type === 'select')
                next = c.selected
                  ? [...new Set([...next, c.id])]
                  : next.filter((id) => id !== c.id);
            return next;
          });
        }}
        onConnect={connect}
        isValidConnection={(c) => !!connectionResult(c).graph}
        onConnectEnd={(event, state) => {
          if (!state.isValid && state.fromNode && state.fromHandle?.type === 'source') {
            if (state.toNode) {
              if (state.toHandle)
                setError(
                  connectionResult({
                    source: state.fromNode.id,
                    target: state.toNode.id,
                    sourceHandle: state.fromHandle.id ?? null,
                    targetHandle: state.toHandle.id ?? null,
                  }).error ?? 'Nieprawidłowe połączenie.',
                );
              return;
            }
            const pointer = 'changedTouches' in event ? event.changedTouches[0] : event;
            openMenu(pointer.clientX, pointer.clientY, {
              nodeId: state.fromNode.id,
              port: state.fromHandle.id?.replace(/^out:/, '') ?? 'value',
            });
          }
        }}
        onNodeContextMenu={(event, node) => nodeAction(event, node.data.node)}
        onPaneContextMenu={(event) => {
          event.preventDefault();
          openMenu(event.clientX, event.clientY);
        }}
        onPaneClick={() => setMenu(null)}
        onMoveStart={() => setMenu(null)}
        onDragOver={(event) => {
          event.preventDefault();
          event.dataTransfer.dropEffect = 'copy';
        }}
        onDrop={(event) => {
          event.preventDefault();
          try {
            const payload = JSON.parse(event.dataTransfer.getData(MIME)) as {
              kind: NodeKind;
              componentId?: string;
              revision?: number;
              operation?: Operator;
              source?: Source;
            };
            if (isOperator(payload.operation) && props.onAddOperator) {
              props.onAddOperator(
                payload.operation,
                screenToFlowPosition({ x: event.clientX, y: event.clientY }),
              );
              return;
            }
            if (!Object.hasOwn(kindLabels, payload.kind)) return;
            const component = project.components.find(
              (c) => c.id === payload.componentId && c.revision === payload.revision,
            );
            if (payload.kind === 'component' && !component)
              throw Error('Brak definicji komponentu.');
            add(
              payload.kind,
              component,
              screenToFlowPosition({ x: event.clientX, y: event.clientY }),
              payload.source,
            );
          } catch {
            setError('Nie udało się dodać przeciągniętego komponentu.');
          }
        }}
        fitView
        fitViewOptions={{ padding: 0.22, maxZoom: 1 }}
        minZoom={0.25}
        maxZoom={1.6}
        selectionOnDrag
        selectionMode={SelectionMode.Partial}
        multiSelectionKeyCode={['Control', 'Meta']}
        panOnDrag={[1, 2]}
        deleteKeyCode={['Backspace', 'Delete']}
        defaultEdgeOptions={{ interactionWidth: 24 }}
      >
        <Background variant={BackgroundVariant.Dots} gap={22} size={1} color="#d4d2cd" />
        <Controls showInteractive={false} position="bottom-left" />
        <MiniMap
          position="bottom-right"
          pannable
          zoomable
          nodeColor={(n) => (n.selected ? '#d85b2c' : '#e1ddd5')}
          maskColor="rgba(247,246,242,.7)"
        />
        <Panel position="top-left">
          <div className="af-canvas-label">
            <span className="af-canvas-dot" /> EDYTOR SCENARIUSZA{' '}
            <span>{graph.nodes.length} kroków</span>
          </div>
          <div className="af-flow-help">
            {graph.nodes.some((n) => n.kind === 'trigger')
              ? 'Start → wykonanie po kolei. Nieaktywne gałęzie są pomijane.'
              : 'Starszy tryb: wszystkie kroki. Dodaj Start i połącz ścieżkę wykonania.'}
          </div>
        </Panel>
        <Panel position="top-right">
          <div className="af-graph-toolbar">
            <button title="Dodaj klocek" onClick={(e) => openMenu(e.clientX - 180, e.clientY + 28)}>
              <IconPlus size={15} /> Dodaj krok
            </button>
            <span />
            <button
              disabled={!selectedIds.length}
              onClick={onSaveSelection}
              title="Zapisz zaznaczone kroki jako komponent"
            >
              <IconCube size={16} />
            </button>
            <button
              disabled={!selectedIds.length}
              onClick={onCopy}
              title="Kopiuj zaznaczenie (Ctrl/Cmd+C)"
            >
              <IconCopy size={16} />
            </button>
            <button
              disabled={!canPaste}
              onClick={onPaste}
              title="Wklej klocki z konfiguracją (Ctrl/Cmd+V)"
            >
              <IconClipboard size={16} />
            </button>
            <button
              disabled={!selectedIds.length}
              onClick={onDuplicate}
              title="Duplikuj zaznaczenie (Ctrl/Cmd+D)"
            >
              <IconLayersSubtract size={16} />
            </button>
            <button
              disabled={!selectedIds.length && !selectedEdges.length}
              onClick={removeSelected}
              title="Usuń zaznaczenie"
            >
              <IconTrash size={16} />
            </button>
          </div>
        </Panel>
        {!graph.nodes.length && (
          <Panel position="top-center" className="af-empty-canvas">
            <IconArrowsSplit size={36} stroke={1.2} />
            <h3>Od pierwszego requestu do całego testu</h3>
            <p>Przeciągnij klocek z biblioteki lub dodaj pierwszy krok.</p>
            <button onClick={() => onAdd('request', { x: 100, y: 100 })}>
              <IconPlus size={16} /> Dodaj request HTTP
            </button>
          </Panel>
        )}
        <Panel position="bottom-center">
          <button
            className="af-canvas-hint"
            onClick={() => fitView({ padding: 0.2, duration: 250 })}
          >
            Ctrl + klik: zaznacz wiele · Przeciągnij tło: zaznacz · Dopasuj widok
          </button>
        </Panel>
      </ReactFlow>
      {error && (
        <div className="af-graph-error" role="alert">
          {error}
          <button onClick={() => setError('')} aria-label="Zamknij komunikat">
            <IconX size={15} />
          </button>
        </div>
      )}
      {menu && (
        <div
          className="af-action-menu nodrag"
          style={{ left: menu.left, top: menu.top }}
          onKeyDown={(e) => {
            if (e.key === 'Escape') setMenu(null);
          }}
        >
          <div className="af-action-menu-head">
            <div>
              <small>{sourceNode ? `DALEJ PO: ${sourceNode.name}` : 'BIBLIOTEKA KROKÓW'}</small>
              <strong>
                {sourceNode ? 'Co chcesz zrobić z danymi?' : 'Dodaj krok do scenariusza'}
              </strong>
            </div>
            <button
              className="af-icon-button"
              onClick={() => setMenu(null)}
              aria-label="Zamknij menu"
            >
              <IconX size={16} />
            </button>
          </div>
          {sourceNode && (
            <label className="af-menu-output">
              Dane z wyjścia
              <select
                value={menu.source?.port}
                onChange={(e) =>
                  setMenu({ ...menu, source: { nodeId: sourceNode.id, port: e.target.value } })
                }
              >
                {outputPorts(sourceNode, project).map((p) => (
                  <option value={p.name} key={p.name}>
                    {p.label ?? p.name} · {p.type}
                  </option>
                ))}
              </select>
            </label>
          )}
          <input
            autoFocus
            className="af-menu-search"
            placeholder="Znajdź akcję…"
            value={search}
            onChange={(e) => setSearch(e.target.value)}
            aria-label="Szukaj akcji"
          />
          <div className="af-action-list">
            {actions
              .filter((a) =>
                `${a.name} ${a.description}`
                  .toLocaleLowerCase()
                  .includes(search.toLocaleLowerCase()),
              )
              .map((action) => {
                const Icon = icons[action.kind];
                return (
                  <button
                    key={action.component?.id ?? action.kind}
                    draggable
                    onDragStart={(event) => {
                      event.dataTransfer.setData(
                        MIME,
                        JSON.stringify({
                          kind: action.kind,
                          componentId: action.component?.id,
                          revision: action.component?.revision,
                          source: menu.source,
                        }),
                      );
                      event.dataTransfer.effectAllowed = 'copy';
                    }}
                    onClick={() =>
                      add(
                        action.kind,
                        action.component,
                        sourceNode
                          ? { x: sourceNode.position.x + 340, y: sourceNode.position.y + 20 }
                          : menu.position,
                      )
                    }
                  >
                    <span className={`af-action-icon af-kind-${action.kind}`}>
                      <Icon size={19} />
                    </span>
                    <span>
                      <strong>{action.name}</strong>
                      <small>{action.description}</small>
                    </span>
                    <IconPlus size={15} />
                  </button>
                );
              })}
          </div>
          <div className="af-menu-hint">Kliknij, aby dodać · przeciągnij w wybrane miejsce</div>
        </div>
      )}
    </div>
  );
}
export function GraphCanvas(props: GraphCanvasProps) {
  return (
    <ReactFlowProvider>
      <Canvas {...props} />
    </ReactFlowProvider>
  );
}

import { AssignResponseVariable } from '../variables/AssignResponseVariable';
import { assignResponseVariable, variablePosition } from '../variables/operations';
import { pruneTemplateInputs } from '../../../shared/variables';
import { VariablePalette } from '../variables/VariablePalette';
import { ResponseDataContext } from '../inspector/responseData';
import { OperatorPalette } from '../operators/OperatorPalette';
import { operators, expression, type Operator } from '../../../shared/operators';
import { references } from '../../../shared/bindings';
import { useEffect, useRef, useState } from 'react';
import {
  IconArrowBackUp,
  IconArrowForwardUp,
  IconArrowLeft,
  IconArrowRight,
  IconBraces,
  IconCheck,
  IconChevronDown,
  IconChevronRight,
  IconCircle,
  IconCube,
  IconDownload,
  IconExternalLink,
  IconFilter,
  IconFlask,
  IconFolder,
  IconGitBranch,
  IconMathFunction,
  IconPlayerPlay,
  IconPlayerStop,
  IconPlus,
  IconSearch,
  IconSettings,
  IconTrash,
  IconUpload,
  IconWorld,
  IconX,
} from '@tabler/icons-react';
import {
  createBlocksProject,
  createWorkflowProject,
  createEmptyProject,
  createShopProject,
  createTicketsProject,
  emptyScenario,
} from '../../../shared/demos';
import {
  createNode,
  definition,
  hasCycle,
  kindDescriptions,
  kindLabels,
  validateProject,
} from '../../../shared/graph';
import { literal, reference, uid } from '../../../shared/model';
import type {
  ComponentDefinition,
  Graph,
  Json,
  NodeKind,
  Project,
  RunRequest,
  RunResult,
  TestNode,
} from '../../../shared/model';
import { createFieldAssertion } from '../graph/assertions';
import { copySelection, pasteSelection } from '../graph/operations';
import { GraphCanvas } from '../graph/GraphCanvas';
import { NodeInspector } from '../inspector/NodeInspector';
import { ComponentBuilder } from '../components/ComponentBuilder';
import { EnvironmentEditor } from '../environments/EnvironmentEditor';
import { RunPanel } from '../runs/RunPanel';
import { ResizableRunPanel } from '../runs/ResizableRunPanel';
import { ProjectSettings } from './ProjectSettings';
import { downloadJson, parseProject, parseWorkspace, publicProject, STORAGE_KEY } from './storage';
import type { Workspace as WorkspaceData } from './storage';
import { jsonRequest } from './http';
import './workspace.css';

const builtinKinds: NodeKind[] = [
  'trigger',
  'if',
  'request',
  'select',
  'build',
  'formula',
  'assert',
  'condition',
];
const icons = {
  trigger: IconCircle,
  if: IconGitBranch,
  variable: IconCircle,
  request: IconWorld,
  select: IconFilter,
  build: IconBraces,
  formula: IconMathFunction,
  assert: IconCheck,
  condition: IconGitBranch,
  component: IconCube,
};
interface Draft {
  definition: ComponentDefinition;
  graph: Graph;
}
interface HistoryEntry {
  graph: Graph;
  cleanup?: Graph;
}
const initialWorkspace = (): WorkspaceData => ({
  version: 1,
  activeId: 'shop',
  projects: [createShopProject(), createTicketsProject()],
});
export function Workspace() {
  const [workspace, setWorkspace] = useState<WorkspaceData>(initialWorkspace);
  const [ready, setReady] = useState(false),
    [canPersist, setCanPersist] = useState(false),
    [saveStatus, setSaveStatus] = useState('Ładowanie…');
  const [scenarioId, setScenarioId] = useState('promotion'),
    [phase, setPhase] = useState<'main' | 'cleanup'>('main');
  const [blockClipboard, setBlockClipboard] = useState<{
    context: string;
    fragment: Graph;
    pastes: number;
  } | null>(null);
  const [selectedIds, setSelectedIds] = useState<string[]>([]),
    [drafts, setDrafts] = useState<Draft[]>([]);
  const [modal, setModal] = useState<'environment' | 'settings' | 'component' | null>(null),
    [builderRevision, setBuilderRevision] = useState(false);
  const [variableCapture, setVariableCapture] = useState<{
    nodeId: string;
    port: string;
    path: string;
    section: 'main' | 'cleanup' | 'draft';
  } | null>(null);
  const [newItem, setNewItem] = useState<{ kind: 'project' | 'scenario'; name: string } | null>(
    null,
  );
  const [notice, setNotice] = useState(''),
    [search, setSearch] = useState(''),
    [secrets, setSecrets] = useState<Record<string, string>>({});
  const [mode, setMode] = useState<'simulation' | 'live'>('simulation'),
    [run, setRun] = useState<RunResult | null>(null),
    [starting, setStarting] = useState(false);
  const [reportOpen, setReportOpen] = useState(true),
    [moreRun, setMoreRun] = useState(false),
    [runFingerprint, setRunFingerprint] = useState('');
  const [, refreshHistory] = useState(0);
  const history = useRef<{
    past: HistoryEntry[];
    future: HistoryEntry[];
    key: string;
    time: number;
  }>({ past: [], future: [], key: '', time: 0 });
  const inputFile = useRef<HTMLInputElement>(null);
  const project =
    workspace.projects.find((p) => p.id === workspace.activeId) ?? workspace.projects[0];
  const scenario = project.scenarios.find((s) => s.id === scenarioId) ?? project.scenarios[0];
  const draft = drafts[drafts.length - 1];
  const graph = draft?.graph ?? (phase === 'main' ? scenario.graph : scenario.cleanup);
  const selected = graph.nodes.find((n) => n.id === selectedIds[0]);
  const running = starting || run?.status === 'running';
  const inspectorGraph =
    phase === 'cleanup' && !draft
      ? {
          nodes: [...scenario.graph.nodes, ...scenario.cleanup.nodes],
          edges: [...scenario.graph.edges, ...scenario.cleanup.edges],
        }
      : graph;
  const latestComponents = project.components.filter(
    (c) => !project.components.some((p) => p.id === c.id && p.revision > c.revision),
  );
  const context = `${project.id}:${scenario.id}:${phase}:${drafts.map((d) => `${d.definition.id}@${d.definition.revision}`).join('/')}`;

  useEffect(() => {
    try {
      const stored = localStorage.getItem(STORAGE_KEY);
      if (stored) {
        const value = parseWorkspace(stored);
        setWorkspace(value);
        setScenarioId(value.projects.find((p) => p.id === value.activeId)!.scenarios[0].id);
      }
      setCanPersist(true);
      setSaveStatus('Zapis lokalny');
    } catch (error) {
      setNotice(
        `Nie można odczytać zapisu: ${error instanceof Error ? error.message : 'Błąd danych'}. Zachowuję go do czasu Twojej pierwszej zmiany projektu.`,
      );
      setSaveStatus('Zapis wymaga uwagi');
    }
    setReady(true);
  }, []);
  useEffect(() => {
    if (!ready || !canPersist) return;
    const persist = () => {
      try {
        localStorage.setItem(
          STORAGE_KEY,
          JSON.stringify({ ...workspace, projects: workspace.projects.map(publicProject) }),
        );
        setSaveStatus('Zapisano lokalnie');
      } catch {
        setSaveStatus('Błąd zapisu');
        setNotice(
          'Nie udało się zapisać projektu w przeglądarce. Wyeksportuj projekt, aby zachować zmiany.',
        );
      }
    };
    const timer = setTimeout(persist, 350);
    // Flush edits when reloading before the debounce interval has elapsed.
    window.addEventListener('pagehide', persist);
    return () => {
      clearTimeout(timer);
      window.removeEventListener('pagehide', persist);
    };
  }, [workspace, ready, canPersist]);
  useEffect(() => {
    setVariableCapture(null);
    history.current = { past: [], future: [], key: '', time: 0 };
    refreshHistory((n) => n + 1);
    setSelectedIds((previous) =>
      previous.filter((id) => graph.nodes.some((node) => node.id === id)),
    );
  }, [context]);
  useEffect(() => {
    if (!run || run.status !== 'running') return;
    let cancelled = false,
      timer: ReturnType<typeof setTimeout>;
    const poll = async () => {
      try {
        const result = await jsonRequest<RunResult>(`/api/runs/${encodeURIComponent(run.id)}`);
        if (!cancelled) {
          setRun(result);
          if (result.status === 'running') timer = setTimeout(poll, 200);
        }
      } catch (error) {
        if (!cancelled) {
          setNotice(error instanceof Error ? error.message : 'Błąd odczytu przebiegu.');
          setRun((previous) =>
            previous
              ? {
                  ...previous,
                  status: 'failed',
                  error:
                    'Utracono połączenie z lokalnym runnerem. Sprzątanie może nadal trwać na serwerze.',
                }
              : null,
          );
        }
      }
    };
    timer = setTimeout(poll, 100);
    return () => {
      cancelled = true;
      clearTimeout(timer);
    };
  }, [run?.id, run?.status]);

  const changeProject = (change: (current: Project) => Project) => {
    setCanPersist(true);
    setSaveStatus('Zapisywanie…');
    setWorkspace((previous) => ({
      ...previous,
      projects: previous.projects.map((p) => (p.id === project.id ? change(p) : p)),
    }));
  };
  const replaceGraph = (next: Graph, restoredCleanup?: Graph) => {
    if (draft) {
      setDrafts((previous) =>
        previous.map((d, i) => (i === previous.length - 1 ? { ...d, graph: next } : d)),
      );
      return;
    }
    changeProject((p) => ({
      ...p,
      scenarios: p.scenarios.map((s) => {
        if (s.id !== scenario.id) return s;
        if (phase === 'cleanup') return { ...s, cleanup: next };
        const removed = s.graph.nodes
          .filter((n) => !next.nodes.some((m) => m.id === n.id))
          .map((n) => n.id);
        return {
          ...s,
          graph: next,
          cleanup: restoredCleanup ?? {
            ...s.cleanup,
            nodes: s.cleanup.nodes.map((n) => ({
              ...n,
              inputs: Object.fromEntries(
                Object.entries(n.inputs).filter(
                  ([, binding]) => !references(binding).some((ref) => removed.includes(ref.nodeId)),
                ),
              ),
            })),
          },
        };
      }),
    }));
  };
  const changeGraph = (next: Graph) => {
    if (JSON.stringify(next) === JSON.stringify(graph)) return;
    if (hasCycle(next)) {
      setNotice('Ta zmiana tworzy cykl zależności. Połączenie nie zostało zapisane.');
      return;
    }
    // Merge consecutive edits to one node into one undo entry (typing or dragging).
    const changed = next.nodes.filter(
      (n) => JSON.stringify(n) !== JSON.stringify(graph.nodes.find((old) => old.id === n.id)),
    );
    const key =
      changed.length === 1 &&
      graph.nodes.length === next.nodes.length &&
      JSON.stringify(graph.edges) === JSON.stringify(next.edges)
        ? changed[0].id
        : '';
    const now = Date.now(),
      h = history.current;
    if (!key || key !== h.key || now - h.time > 650)
      h.past = [
        ...h.past.slice(-49),
        {
          graph: structuredClone(graph),
          cleanup: !draft && phase === 'main' ? structuredClone(scenario.cleanup) : undefined,
        },
      ];
    h.future = [];
    h.key = key;
    h.time = now;
    refreshHistory((n) => n + 1);
    replaceGraph(next);
  };
  const undo = () => {
    const h = history.current,
      previous = h.past.at(-1);
    if (!previous) return;
    h.past = h.past.slice(0, -1);
    h.future = [
      {
        graph: structuredClone(graph),
        cleanup: !draft && phase === 'main' ? structuredClone(scenario.cleanup) : undefined,
      },
      ...h.future,
    ].slice(0, 50);
    h.key = '';
    replaceGraph(previous.graph, previous.cleanup);
    refreshHistory((n) => n + 1);
  };
  const redo = () => {
    const h = history.current,
      next = h.future[0];
    if (!next) return;
    h.past = [
      ...h.past,
      {
        graph: structuredClone(graph),
        cleanup: !draft && phase === 'main' ? structuredClone(scenario.cleanup) : undefined,
      },
    ];
    h.future = h.future.slice(1);
    h.key = '';
    replaceGraph(next.graph, next.cleanup);
    refreshHistory((n) => n + 1);
  };
  const copyBlocks = () => {
    const fragment = copySelection(graph, selectedIds);
    if (!fragment.nodes.length) return;
    setBlockClipboard({ context, fragment, pastes: 0 });
    setNotice(
      `Skopiowano klocki: ${fragment.nodes.length}. Wklej przez Ctrl/Cmd+V lub przycisk Wklej.`,
    );
  };
  const insertBlocks = (fragment: Graph, offset = { x: 40, y: 50 }) => {
    const result = pasteSelection(
      graph,
      fragment,
      offset,
      inspectorGraph.nodes.map((node) => node.id),
    );
    if (result.error) {
      setNotice(result.error);
      return false;
    }
    if (!result.ids.length) return false;
    changeGraph(result.graph);
    setSelectedIds(result.ids);
    setNotice(`Wklejono klocki z konfiguracją: ${result.ids.length}.`);
    return true;
  };
  const pasteBlocks = () => {
    if (!blockClipboard || blockClipboard.context !== context) return;
    const count = blockClipboard.pastes + 1;
    if (insertBlocks(blockClipboard.fragment, { x: 40 * count, y: 50 * count }))
      setBlockClipboard({ ...blockClipboard, pastes: count });
  };
  const duplicateBlocks = () => insertBlocks(copySelection(graph, selectedIds));
  useEffect(() => {
    const listener = (event: KeyboardEvent) => {
      const target = event.target as HTMLElement;
      if (
        target.closest('input,textarea,select,[contenteditable="true"],[role="dialog"]') ||
        modal ||
        newItem ||
        variableCapture
      )
        return;
      // Text fields retain native copy/paste. Board shortcuts use a frozen graph fragment.
      if ((event.ctrlKey || event.metaKey) && !event.altKey) {
        const key = event.key.toLowerCase();
        if (key === 'c' && selectedIds.length && !window.getSelection()?.toString()) {
          event.preventDefault();
          copyBlocks();
        }
        if (key === 'v' && blockClipboard?.context === context) {
          event.preventDefault();
          pasteBlocks();
        }
        if (key === 'd' && selectedIds.length) {
          event.preventDefault();
          duplicateBlocks();
        }
      }
      if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === 'z') {
        event.preventDefault();
        event.shiftKey ? redo() : undo();
      }
      if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === 'y') {
        event.preventDefault();
        redo();
      }
    };
    window.addEventListener('keydown', listener);
    return () => window.removeEventListener('keydown', listener);
  });

  const addNode = (
    kind: NodeKind,
    position: { x: number; y: number },
    component?: ComponentDefinition,
    source?: { nodeId: string; port: string },
  ) => {
    if (kind === 'trigger' && graph.nodes.some((n) => n.kind === 'trigger')) {
      setNotice('Scenariusz ma już klocek Start.');
      return;
    }
    const node = createNode(kind, position, component);
    if (kind === 'variable')
      node.name = `Zmienna ${graph.nodes.filter((node) => node.kind === 'variable').length + 1}`;
    let edges = graph.edges;
    if (source && ['__order', '__true', '__false'].includes(source.port)) {
      edges = [
        ...edges,
        {
          id: uid('order'),
          source: source.nodeId,
          target: node.id,
          ...(source.port === '__true'
            ? { branch: 'true' as const }
            : source.port === '__false'
              ? { branch: 'false' as const }
              : {}),
        },
      ];
    } else if (source) {
      const input =
        component?.action?.input ??
        {
          trigger: '',
          if: 'actual',
          variable: 'value',
          request: 'body',
          select: 'value',
          assert: 'actual',
          condition: 'actual',
          formula: 'left',
          build: 'field',
          component: '',
        }[kind];
      if (input) node.inputs[input] = reference(source.nodeId, source.port);
      else edges = [...edges, { id: uid('order'), source: source.nodeId, target: node.id }];
    }
    // The first executable block gets a visible connection from the new scenario's Start.
    const start = graph.nodes.find((n) => n.kind === 'trigger');
    if (
      !source &&
      start &&
      ['request', 'component'].includes(kind) &&
      !edges.some((e) => e.source === start.id)
    )
      edges = [...edges, { id: uid('order'), source: start.id, target: node.id }];
    changeGraph({ nodes: [...graph.nodes, node], edges });
    setSelectedIds([node.id]);
  };
  const openBlocksExample = () => {
    if (running) return;
    const existing = workspace.projects.find((p) => p.id === 'blocks-poc');
    if (existing) {
      switchProject(existing.id);
      return;
    }
    const created = createBlocksProject();
    setCanPersist(true);
    setWorkspace((previous) => ({
      ...previous,
      activeId: created.id,
      projects: [...previous.projects, created],
    }));
    setScenarioId(created.scenarios[0].id);
    setDrafts([]);
    setPhase('main');
    setRun(null);
    setSecrets({});
    setNotice(
      'Przykład: otwórz Porównanie i edytuj zagnieżdżone bloki w jego argumentach. Możesz uruchomić symulację lub lokalne API.',
    );
  };
  const openWorkflowExample = () => {
    if (running) return;
    const existing = workspace.projects.find((p) => p.id === 'workflow-poc');
    if (existing) {
      switchProject(existing.id);
      return;
    }
    const created = createWorkflowProject();
    setCanPersist(true);
    setWorkspace((previous) => ({
      ...previous,
      activeId: created.id,
      projects: [...previous.projects, created],
    }));
    setScenarioId(created.scenarios[0].id);
    setDrafts([]);
    setPhase('main');
    setRun(null);
    setSecrets({});
    setNotice(
      'Start → ceny → IF → koszyk → dodanie produktów. Wybierz API i uruchom test; gałąź Nie nie tworzy koszyka.',
    );
  };
  const addOperator = (operator: Operator, position = { x: 120, y: 120 }) => {
    const node = createNode('formula', position);
    node.name = operators[operator].label;
    node.config.operation = operator;
    node.inputs = expression(operator).inputs;
    changeGraph({ ...graph, nodes: [...graph.nodes, node] });
    setSelectedIds([node.id]);
  };
  const addAssertion = (nodeId: string, port: string, path: string, value: Json) => {
    const source = inspectorGraph.nodes.find((n) => n.id === nodeId);
    if (!source) return;
    const node = createFieldAssertion(source, port, path, value);
    changeGraph({ ...graph, nodes: [...graph.nodes, node] });
    setSelectedIds([node.id]);
    setNotice(
      `Dodano porównanie „${path || port}”. Sprawdź wartość lub typ w konfiguracji asercji.`,
    );
  };
  const addReportAssertion = (nodeId: string, port: string, path: string, value: Json) => {
    if (runFingerprint !== JSON.stringify(publicProject(project))) {
      setNotice(
        'Graf zmieniono po wykonaniu. Uruchom test ponownie, aby dodać porównanie z aktualnych danych.',
      );
      return;
    }
    // A report may point to cleanup while the editor currently shows the main graph.
    const targetPhase = scenario.graph.nodes.some((n) => n.id === nodeId) ? 'main' : 'cleanup';
    const target = targetPhase === 'main' ? scenario.graph : scenario.cleanup;
    const source = target.nodes.find((n) => n.id === nodeId);
    if (!source) return;
    if (targetPhase === phase) {
      addAssertion(nodeId, port, path, value);
      return;
    }
    const node = createFieldAssertion(source, port, path, value);
    changeProject((p) => ({
      ...p,
      scenarios: p.scenarios.map((s) =>
        s.id === scenario.id
          ? {
              ...s,
              [targetPhase === 'main' ? 'graph' : 'cleanup']: {
                ...target,
                nodes: [...target.nodes, node],
              },
            }
          : s,
      ),
    }));
    setPhase(targetPhase);
    setSelectedIds([node.id]);
    setNotice(`Dodano porównanie „${path || port}”. Sprawdź wartość oczekiwaną pobraną z raportu.`);
  };
  const captureResponseField = (nodeId: string, port: string, path: string, fromReport = false) => {
    const section =
      draft && !fromReport
        ? 'draft'
        : scenario.graph.nodes.some((node) => node.id === nodeId)
          ? 'main'
          : 'cleanup';
    const sourceGraph =
      section === 'draft' ? draft!.graph : section === 'main' ? scenario.graph : scenario.cleanup;
    if (!sourceGraph.nodes.some((node) => node.id === nodeId)) {
      setNotice('Klocek źródłowy nie jest już dostępny.');
      return;
    }
    setVariableCapture({ nodeId, port, path, section });
  };
  const captureGraph =
    variableCapture?.section === 'draft'
      ? draft?.graph
      : variableCapture?.section === 'main'
        ? scenario.graph
        : scenario.cleanup;
  const captureSource = captureGraph?.nodes.find((node) => node.id === variableCapture?.nodeId);
  const saveResponseVariable = (name: string, targetId?: string): string | undefined => {
    if (!variableCapture || !captureGraph) return 'Nie ma już źródła odpowiedzi.';
    const result = assignResponseVariable(
      captureGraph,
      project,
      variableCapture.nodeId,
      variableCapture.port,
      variableCapture.path,
      name,
      targetId,
    );
    if (!result.graph) return result.error;
    if (variableCapture.section === 'draft' || (variableCapture.section === phase && !draft))
      changeGraph(result.graph);
    else
      changeProject((project) => ({
        ...project,
        scenarios: project.scenarios.map((s) =>
          s.id === scenario.id
            ? { ...s, [variableCapture.section === 'main' ? 'graph' : 'cleanup']: result.graph! }
            : s,
        ),
      }));
    if (variableCapture.section !== 'draft') setPhase(variableCapture.section);
    setSelectedIds([result.variableId!]);
    setVariableCapture(null);
    setNotice(
      `Zmienna „${name}” pobiera ${variableCapture.path || variableCapture.port} z odpowiedzi przy każdym wykonaniu.`,
    );
  };
  const editComponent = (node: TestNode) => {
    const component = definition(project, node);
    if (!component) {
      setNotice('Brak zapisanej rewizji komponentu.');
      return;
    }
    setDrafts((previous) => [
      ...previous,
      { definition: component, graph: structuredClone(component.graph) },
    ]);
    setSelectedIds([]);
  };
  const saveComponent = (component: ComponentDefinition) => {
    // Save a new revision without silently changing existing component instances.
    const saved =
      builderRevision && draft
        ? {
            ...component,
            id: draft.definition.id,
            revision:
              Math.max(
                ...project.components
                  .filter((c) => c.id === draft.definition.id)
                  .map((c) => c.revision),
              ) + 1,
          }
        : component;
    const next = { ...project, components: [...project.components, saved] },
      errors = validateProject(next);
    if (errors.length) return errors.join(' ');
    changeProject(() => next);
    setModal(null);
    if (builderRevision) setDrafts((previous) => previous.slice(0, -1));
    setNotice(
      `Zapisano „${saved.name}”, rewizja ${saved.revision}. Istniejące instancje zachowują swoje rewizje.`,
    );
  };
  const switchProject = (id: string) => {
    const next = workspace.projects.find((p) => p.id === id);
    if (!next || running) return;
    setWorkspace((previous) => ({ ...previous, activeId: id }));
    setScenarioId(next.scenarios[0].id);
    setDrafts([]);
    setPhase('main');
    setRun(null);
    setSecrets({});
    setNotice('');
  };
  const switchScenario = (id: string) => {
    if (running) return;
    setScenarioId(id);
    setDrafts([]);
    setPhase('main');
    setRun(null);
  };
  const newProjectOrScenario = () => {
    if (!newItem?.name.trim()) return;
    setCanPersist(true);
    if (newItem.kind === 'project') {
      const created = createEmptyProject();
      created.name = newItem.name.trim();
      setWorkspace((previous) => ({
        ...previous,
        activeId: created.id,
        projects: [...previous.projects, created],
      }));
      setScenarioId(created.scenarios[0].id);
      setSecrets({});
    } else {
      const created = emptyScenario(newItem.name.trim());
      changeProject((p) => ({ ...p, scenarios: [...p.scenarios, created] }));
      setScenarioId(created.id);
    }
    setNewItem(null);
    setRun(null);
    setDrafts([]);
    setPhase('main');
  };
  const importProject = async (file: File) => {
    try {
      if (file.size > 4 * 1024 * 1024) throw new Error('Plik przekracza limit 4 MiB.');
      const imported = parseProject(await file.text());
      const existing = workspace.projects.some((p) => p.id === imported.id);
      const added = existing
        ? { ...imported, id: uid('project'), name: `${imported.name} (import)` }
        : imported;
      setCanPersist(true);
      setWorkspace((previous) => ({
        ...previous,
        activeId: added.id,
        projects: [...previous.projects, added],
      }));
      setScenarioId(added.scenarios[0].id);
      setSecrets({});
      setRun(null);
      setDrafts([]);
      setPhase('main');
      setNotice('Projekt został zaimportowany.');
    } catch (error) {
      setNotice(
        `Import nie powiódł się: ${error instanceof Error ? error.message : 'Nieprawidłowy plik'}`,
      );
    }
  };
  const startRun = async (kind: 'full' | 'until' | 'single' = 'full') => {
    if (running || draft || (kind !== 'full' && !selected)) return;
    if (
      kind === 'single' &&
      run &&
      (run.mode !== mode || runFingerprint !== JSON.stringify(publicProject(project)))
    ) {
      setNotice(
        'Poprzednie dane pochodzą z innego trybu lub wcześniejszej konfiguracji. Uruchom test do wybranego klocka, aby pobrać świeże wejścia.',
      );
      setMoreRun(false);
      return;
    }
    const errors = validateProject(project);
    if (errors.length) {
      setNotice(errors.join(' '));
      return;
    }
    setStarting(true);
    setMoreRun(false);
    setNotice('');
    setReportOpen(true);
    const payload: RunRequest = {
      project: structuredClone(project),
      scenarioId: scenario.id,
      mode,
      secrets,
      ...(kind === 'until' ? { targetNodeId: selected!.id } : {}),
      ...(kind === 'single'
        ? { singleNodeId: selected!.id, ...(run ? { previousRun: run } : {}) }
        : {}),
    };
    try {
      const health = await jsonRequest<{ service?: string }>('/api/health');
      if (health?.service !== 'apiforge-runner')
        throw new Error(
          'Pod adresem runnera działa inna usługa. Uruchom lokalne API Forge na porcie 4318.',
        );
      const { id } = await jsonRequest<{ id: string }>('/api/runs', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(payload),
      });
      if (typeof id !== 'string' || !id)
        throw new Error('Runner nie zwrócił identyfikatora przebiegu.');
      setRun(await jsonRequest<RunResult>(`/api/runs/${encodeURIComponent(id)}`));
      setRunFingerprint(JSON.stringify(publicProject(project)));
    } catch (error) {
      setNotice(error instanceof Error ? error.message : 'Nie udało się uruchomić testu.');
    } finally {
      setStarting(false);
    }
  };
  const cancelRun = async () => {
    if (!run) return;
    try {
      await jsonRequest(`/api/runs/${encodeURIComponent(run.id)}/cancel`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: '{}',
      });
      setNotice('Wysłano anulowanie. Oczekiwanie na zakończenie kroku i sprzątania.');
    } catch (error) {
      setNotice(error instanceof Error ? error.message : 'Błąd anulowania.');
    }
  };
  const selectResult = (id: string) => {
    const step = run?.steps.find((s) => s.nodeId === id);
    if (!step) return;
    setDrafts([]);
    setPhase(step.phase === 'cleanup' ? 'cleanup' : 'main');
    setSelectedIds([id]);
  };
  const stale = run && runFingerprint !== JSON.stringify(publicProject(project));
  const builderGraph = graph;
  const resultSteps = !draft ? run?.steps.filter((s) => s.phase === phase) : undefined;
  const environmentLabel = (() => {
    try {
      return new URL(project.environment.baseUrl).host;
    } catch {
      return 'Skonfiguruj środowisko';
    }
  })();
  const drawLibrary = (kind: NodeKind, component?: ComponentDefinition) => {
    const Icon = icons[kind],
      name = component?.name ?? kindLabels[kind],
      description = component?.description ?? kindDescriptions[kind];
    if (!`${name} ${description}`.toLocaleLowerCase().includes(search.toLocaleLowerCase()))
      return null;
    return (
      <button
        key={component?.id ?? kind}
        className={`af-library-item af-library-${kind}`}
        draggable
        onDragStart={(e) => {
          e.dataTransfer.setData(
            'application/apiforge-node',
            JSON.stringify({ kind, componentId: component?.id, revision: component?.revision }),
          );
          e.dataTransfer.effectAllowed = 'copy';
        }}
        onClick={() => addNode(kind, variablePosition(graph), component)}
        title={`${description} — kliknij lub przeciągnij`}
      >
        <span>
          <Icon size={17} stroke={1.6} />
        </span>
        <div>
          <strong>{name}</strong>
          <small>{component ? `Komponent · r${component.revision}` : description}</small>
        </div>
        <IconPlus size={12} className="af-library-plus" />
      </button>
    );
  };

  return (
    <div className="af-workspace">
      <aside className="af-sidebar">
        <a className="af-brand" href="/" aria-label="API Forge">
          <span>
            <IconGitBranch size={23} stroke={2} />
          </span>
          <strong>
            api<span>forge</span>
          </strong>
          <small>PROTOTYPE</small>
        </a>
        <div className="af-project-picker">
          <label>OBSZAR ROBOCZY</label>
          <div>
            <IconFolder size={16} />
            <select
              aria-label="Wybierz projekt"
              value={project.id}
              disabled={running}
              onChange={(e) => switchProject(e.target.value)}
            >
              {workspace.projects.map((p) => (
                <option key={p.id} value={p.id}>
                  {p.name}
                </option>
              ))}
            </select>
            <button
              title="Nowy projekt"
              disabled={running}
              onClick={() => setNewItem({ kind: 'project', name: '' })}
            >
              <IconPlus size={14} />
            </button>
          </div>
        </div>
        <div className="af-sidebar-section">
          <div className="af-sidebar-heading">
            SCENARIUSZE{' '}
            <button
              title="Nowy scenariusz"
              disabled={running}
              onClick={() => setNewItem({ kind: 'scenario', name: '' })}
            >
              <IconPlus size={14} />
            </button>
          </div>
          <nav className="af-scenarios">
            {project.scenarios.map((s) => (
              <button
                key={s.id}
                disabled={running && s.id !== scenario.id}
                className={s.id === scenario.id ? 'active' : ''}
                onClick={() => switchScenario(s.id)}
              >
                <IconGitBranch size={15} />
                <span>{s.name}</span>
                {s.id === scenario.id && <span className="af-scenario-dot" />}
              </button>
            ))}
          </nav>
        </div>
        <div className="af-sidebar-library">
          <div className="af-sidebar-heading">
            BIBLIOTEKA KLOCKÓW <span>{builtinKinds.length + latestComponents.length}</span>
          </div>
          <div className="af-library-search">
            <IconSearch size={13} />
            <input
              aria-label="Szukaj klocka"
              placeholder="Szukaj klocka…"
              value={search}
              onChange={(e) => setSearch(e.target.value)}
            />
          </div>
          <div className="af-library-scroll">
            <button className="af-workflow-example" onClick={openWorkflowExample}>
              Przykład: kolejność i IF
            </button>
            <p className="af-library-category">OPERACJE PODSTAWOWE</p>
            {builtinKinds.map((kind) => drawLibrary(kind))}
            <VariablePalette
              graph={inspectorGraph}
              project={project}
              parameters={
                draft
                  ? Object.fromEntries(
                      draft.definition.inputs.map((port) => [port.name, port.defaultValue ?? null]),
                    )
                  : scenario.parameters
              }
              search={search}
              onSelect={(id) => setSelectedIds([id])}
              onAdd={() => addNode('variable', variablePosition(graph))}
            />
            <OperatorPalette
              search={search}
              onAdd={(operator) => addOperator(operator)}
              onExample={openBlocksExample}
            />
            <div className="af-library-category af-custom-category">
              KOMPONENTY PROJEKTU <span>{latestComponents.length}</span>
            </div>
            {latestComponents.map((c) => drawLibrary('component', c))}
            {!latestComponents.length && (
              <p className="af-library-empty">
                Zaznacz kroki na grafie i zapisz je jako własny komponent.
              </p>
            )}
          </div>
        </div>
        <button className="af-environment-button" onClick={() => setModal('environment')}>
          <span className="af-env-dot" />
          <div>
            <small>ŚRODOWISKO</small>
            <strong>{environmentLabel}</strong>
          </div>
          <IconSettings size={15} />
        </button>
        <div className="af-local-status">
          <span
            className={saveStatus.includes('Błąd') || saveStatus.includes('uwagi') ? 'warn' : ''}
          />
          {saveStatus}
          <span>v0.1</span>
        </div>
      </aside>
      <main className="af-main">
        <header className="af-topbar">
          <div className="af-breadcrumb">
            <IconFolder size={14} />
            <span>{project.name}</span>
            <IconChevronRight size={12} />
            <strong>{draft ? 'Edytor komponentu' : 'Scenariusze'}</strong>
          </div>
          <div className="af-topbar-actions">
            <button
              onClick={() => inputFile.current?.click()}
              disabled={running}
              title="Importuj projekt JSON"
            >
              <IconUpload size={14} />
              Import
            </button>
            <button
              onClick={() =>
                downloadJson(
                  `${project.name.replace(/[^\p{L}\p{N}-]/gu, '-')}.json`,
                  publicProject(project),
                )
              }
              title="Eksportuj projekt JSON"
            >
              <IconDownload size={14} />
              Eksport
            </button>
            <span />
            <button onClick={() => setModal('settings')} title="Ustawienia projektu i scenariusza">
              <IconSettings size={16} />
            </button>
          </div>
          <input
            hidden
            ref={inputFile}
            type="file"
            accept="application/json,.json"
            onChange={(e) => {
              const file = e.target.files?.[0];
              if (file) void importProject(file);
              e.target.value = '';
            }}
          />
        </header>
        <section className="af-scenario-bar">
          <div className="af-scenario-info">
            <div className="af-scenario-eyebrow">
              {draft
                ? `KOMPONENT · REWIZJA ${draft.definition.revision} · KOPIA ROBOCZA`
                : 'TEST REGRESJI API'}
              <span>
                {draft
                  ? `${draft.graph.nodes.length} kroków`
                  : `${scenario.graph.nodes.length} kroków · ${scenario.cleanup.nodes.length} po teście`}
              </span>
            </div>
            <h1>{draft?.definition.name ?? scenario.name}</h1>
            <p>
              {draft?.definition.description ??
                (scenario.description || 'Zbuduj scenariusz z requestów, danych i sprawdzeń.')}
            </p>
          </div>
          <div className="af-execution-controls">
            {draft ? (
              <>
                <button
                  className="af-secondary-button"
                  onClick={() => setDrafts((previous) => previous.slice(0, -1))}
                >
                  <IconArrowLeft size={14} />
                  Wróć bez zapisu
                </button>
                <button
                  className="af-primary-button"
                  disabled={!graph.nodes.length}
                  onClick={() => {
                    setBuilderRevision(true);
                    setModal('component');
                  }}
                >
                  <IconCheck size={14} />
                  Zapisz rewizję
                </button>
              </>
            ) : (
              <>
                <div className="af-mode-selector">
                  <button
                    disabled={running}
                    className={mode === 'simulation' ? 'active' : ''}
                    onClick={() => setMode('simulation')}
                  >
                    <IconFlask size={13} />
                    Symulacja
                  </button>
                  <button
                    disabled={running}
                    className={mode === 'live' ? 'active' : ''}
                    onClick={() => setMode('live')}
                  >
                    <IconWorld size={13} />
                    API
                  </button>
                </div>
                {running ? (
                  <button className="af-stop-button" onClick={cancelRun} disabled={!run}>
                    <IconPlayerStop size={14} />
                    Anuluj
                  </button>
                ) : (
                  <div className="af-run-buttons">
                    <button
                      className="af-primary-button"
                      disabled={!ready || !scenario.graph.nodes.length}
                      onClick={() => startRun()}
                    >
                      <IconPlayerPlay size={15} fill="currentColor" />
                      Uruchom test
                    </button>
                    <button
                      className="af-run-dropdown"
                      aria-label="Opcje wykonania"
                      onClick={() => setMoreRun(!moreRun)}
                    >
                      <IconChevronDown size={14} />
                    </button>
                    {moreRun && (
                      <div className="af-run-options">
                        <button
                          disabled={!selected || phase !== 'main'}
                          onClick={() => startRun('until')}
                        >
                          Uruchom do wybranego klocka
                          <small>Świeże dane z poprzedzających kroków</small>
                        </button>
                        <button
                          disabled={!selected || phase !== 'main'}
                          onClick={() => startRun('single')}
                        >
                          Wykonaj tylko wybrany krok
                          <small>
                            {run
                              ? `Użyj dostępnych wejść z ${new Date(run.startedAt).toLocaleTimeString('pl-PL')}`
                              : 'Bieżące literały i argumenty; brak historii'}
                          </small>
                        </button>
                      </div>
                    )}
                  </div>
                )}
              </>
            )}
          </div>
        </section>
        {notice && (
          <div className="af-workspace-notice" role="status">
            <span>{notice}</span>
            <button onClick={() => setNotice('')} aria-label="Zamknij powiadomienie">
              <IconX size={14} />
            </button>
          </div>
        )}
        <div className="af-editor-bar">
          <div className="af-graph-tabs">
            {draft ? (
              <span className="af-draft-breadcrumb">
                {drafts.map((d, i) => (
                  <span key={`${d.definition.id}-${i}`}>
                    {i > 0 && <IconChevronRight size={12} />}
                    <button onClick={() => setDrafts((previous) => previous.slice(0, i + 1))}>
                      {d.definition.name}
                    </button>
                  </span>
                ))}
              </span>
            ) : (
              <>
                <button
                  className={phase === 'main' ? 'active' : ''}
                  onClick={() => {
                    setPhase('main');
                    setSelectedIds([]);
                  }}
                >
                  <IconGitBranch size={14} />
                  Scenariusz
                </button>
                <button
                  className={phase === 'cleanup' ? 'active' : ''}
                  onClick={() => {
                    setPhase('cleanup');
                    setSelectedIds([]);
                  }}
                >
                  <IconTrash size={13} />
                  Po teście <small>{scenario.cleanup.nodes.length}</small>
                </button>
              </>
            )}
          </div>
          <div className="af-editor-tools">
            <span>
              {mode === 'simulation'
                ? 'Dane przykładowe · bez requestów'
                : 'Requesty przez lokalny runner'}
            </span>
            <button title="Cofnij (Ctrl+Z)" disabled={!history.current.past.length} onClick={undo}>
              <IconArrowBackUp size={16} />
            </button>
            <button
              title="Ponów (Ctrl+Shift+Z)"
              disabled={!history.current.future.length}
              onClick={redo}
            >
              <IconArrowForwardUp size={16} />
            </button>
            <button
              className="af-save-component"
              disabled={!selectedIds.length}
              onClick={() => {
                setBuilderRevision(false);
                setModal('component');
              }}
            >
              <IconCube size={13} />
              Zapisz komponent
            </button>
          </div>
        </div>
        <div className={`af-editing-area ${selected ? 'has-inspector' : ''}`}>
          <div className="af-canvas-slot">
            <GraphCanvas
              key={context}
              graph={graph}
              project={project}
              selectedIds={selectedIds}
              onCopy={copyBlocks}
              onPaste={pasteBlocks}
              onDuplicate={duplicateBlocks}
              canPaste={blockClipboard?.context === context}
              onSelect={setSelectedIds}
              onChange={changeGraph}
              onAdd={addNode}
              onAddOperator={addOperator}
              onSaveSelection={() => {
                setBuilderRevision(false);
                setModal('component');
              }}
              onEditComponent={editComponent}
              steps={stale ? undefined : resultSteps}
            />
          </div>
          {selected && (
            <div className="af-inspector-slot">
              <button
                className="af-close-inspector"
                title="Zamknij konfigurację"
                onClick={() => setSelectedIds([])}
              >
                <IconX size={15} />
              </button>
              <ResponseDataContext.Provider value={draft ? null : run}>
                <NodeInspector
                  key={selected.id}
                  node={selected}
                  graph={inspectorGraph}
                  flowGraph={graph}
                  onGraphChange={changeGraph}
                  project={project}
                  onChange={(node) => {
                    const next = pruneTemplateInputs(node);
                    changeGraph({
                      ...graph,
                      nodes: graph.nodes.map((n) => (n.id === next.id ? next : n)),
                    });
                  }}
                  onEditComponent={editComponent}
                  onAssertField={addAssertion}
                  onAssignField={(nodeId, port, path) => captureResponseField(nodeId, port, path)}
                  step={stale ? undefined : resultSteps?.find((s) => s.nodeId === selected.id)}
                  parameters={
                    draft
                      ? Object.fromEntries(
                          draft.definition.inputs.map((p) => [p.name, p.defaultValue ?? null]),
                        )
                      : scenario.parameters
                  }
                />
              </ResponseDataContext.Provider>
            </div>
          )}
        </div>
        {!draft && (
          <>
            <div className="af-report-toggle">
              <button onClick={() => setReportOpen(!reportOpen)}>
                <IconChevronDown
                  size={12}
                  style={{ transform: reportOpen ? 'rotate(0deg)' : 'rotate(-90deg)' }}
                />
                {reportOpen ? 'Ukryj' : 'Pokaż'} przebieg
                {run && (
                  <span className={`af-result-pill ${run.status}`}>
                    {run.status === 'passed'
                      ? 'ZALICZONY'
                      : run.status === 'running'
                        ? 'W TOKU'
                        : run.status === 'cancelled'
                          ? 'ANULOWANY'
                          : 'BŁĄD'}
                  </span>
                )}
              </button>
              <span>
                {stale
                  ? 'Graf zmieniono po wykonaniu — raport przedstawia wcześniejszą wersję'
                  : 'Wyniki i dane każdego kroku'}
                <select
                  aria-label="Polityka błędów"
                  value={scenario.errorPolicy}
                  disabled={running}
                  onChange={(e) =>
                    changeProject((p) => ({
                      ...p,
                      scenarios: p.scenarios.map((s) =>
                        s.id === scenario.id
                          ? { ...s, errorPolicy: e.target.value as 'stop' | 'continue' }
                          : s,
                      ),
                    }))
                  }
                >
                  <option value="continue">Kontynuuj możliwe kroki</option>
                  <option value="stop">Zatrzymaj po błędzie</option>
                </select>
              </span>
            </div>
            <ResizableRunPanel hidden={!reportOpen}>
              <RunPanel
                run={run}
                selectedNodeId={selected?.id}
                onSelect={selectResult}
                onAssertField={addReportAssertion}
                onAssignField={(nodeId, port, path) =>
                  captureResponseField(nodeId, port, path, true)
                }
              />
            </ResizableRunPanel>
          </>
        )}
      </main>
      {variableCapture && captureGraph && captureSource && (
        <AssignResponseVariable
          graph={captureGraph}
          source={captureSource}
          port={variableCapture.port}
          path={variableCapture.path}
          onClose={() => setVariableCapture(null)}
          onSave={saveResponseVariable}
        />
      )}
      {modal === 'environment' && (
        <EnvironmentEditor
          environment={project.environment}
          secrets={secrets}
          onChange={(environment) => changeProject((p) => ({ ...p, environment }))}
          onSecretsChange={setSecrets}
          onClose={() => setModal(null)}
        />
      )}
      {modal === 'settings' && (
        <ProjectSettings
          project={project}
          scenario={scenario}
          onChange={(next) => changeProject(() => next)}
          onClose={() => setModal(null)}
        />
      )}
      {modal === 'component' && (
        <ComponentBuilder
          graph={builderGraph}
          selectedIds={builderRevision ? graph.nodes.map((n) => n.id) : selectedIds}
          project={project}
          initialDefinition={builderRevision ? draft?.definition : undefined}
          onSave={saveComponent}
          onClose={() => setModal(null)}
        />
      )}
      {newItem && (
        <div className="af-modal-backdrop">
          <form
            className="af-new-item"
            onSubmit={(e) => {
              e.preventDefault();
              newProjectOrScenario();
            }}
          >
            <div>
              <h2>{newItem.kind === 'project' ? 'Nowy projekt API' : 'Nowy scenariusz'}</h2>
              <button type="button" onClick={() => setNewItem(null)} aria-label="Zamknij">
                <IconX size={17} />
              </button>
            </div>
            <label>
              Nazwa
              <input
                autoFocus
                value={newItem.name}
                placeholder={
                  newItem.kind === 'project' ? 'np. API platformy' : 'np. Rejestracja użytkownika'
                }
                onChange={(e) => setNewItem({ ...newItem, name: e.target.value })}
              />
            </label>
            <p>
              {newItem.kind === 'project'
                ? 'Własne endpointy, komponenty i środowisko testowe.'
                : 'Zacznij od pustego grafu i klocków z biblioteki projektu.'}
            </p>
            <footer>
              <button
                type="button"
                className="af-secondary-button"
                onClick={() => setNewItem(null)}
              >
                Anuluj
              </button>
              <button className="af-primary-button" disabled={!newItem.name.trim()} type="submit">
                Utwórz
                <IconArrowRight size={14} />
              </button>
            </footer>
          </form>
        </div>
      )}
    </div>
  );
}

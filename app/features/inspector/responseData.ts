import { sampleOutput } from './fields';
import { createContext, useContext } from 'react';
import type { RunResult, Project, TestNode } from '../../../shared/model';

// Scope historical outputs to this inspector, including nested argument editors.
export const ResponseDataContext = createContext<RunResult | null>(null);

export function useResponseSource(source: TestNode | undefined, port: string, project: Project) {
  const run = useContext(ResponseDataContext);
  const step = source ? run?.steps.find((step) => step.nodeId === source.id) : undefined;
  const observed = step && Object.hasOwn(step.outputs, port);
  return {
    sample: observed
      ? step.outputs[port]
      : source
        ? sampleOutput(source, port, project)
        : undefined,
    origin: observed
      ? `Ostatnia odpowiedź · ${run?.mode === 'live' ? 'API' : 'symulacja'}`
      : 'Przykładowe dane',
  };
}

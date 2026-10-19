import { Router } from 'express';
import type { RunResult } from '../../shared/model';
import { runScenario } from '../runner';
import { validateRunRequest } from '../runner/validation';

export function createRunsRouter() {
  const router = Router();
  const runs = new Map<string, { result: RunResult; controller: AbortController }>();
  const sizes = new Map<string, number>();
  function trim() {
    let total = [...sizes.values()].reduce((sum, n) => sum + n, 0);
    for (const [id, entry] of runs) {
      if (total <= 32 * 1024 * 1024 && runs.size <= 100) break;
      if (entry.result.status === 'running') continue;
      total -= sizes.get(id) ?? 0;
      sizes.delete(id);
      runs.delete(id);
    }
  }
  router.post('/', (req, res) => {
    try {
      validateRunRequest(req.body);
    } catch (error) {
      res
        .status(400)
        .json({ error: error instanceof Error ? error.message : 'Nieprawidłowe dane.' });
      return;
    }
    if ([...runs.values()].filter((r) => r.result.status === 'running').length >= 4) {
      res.status(429).json({ error: 'Maksymalnie cztery aktywne przebiegi.' });
      return;
    }
    while (runs.size >= 100) {
      const old = [...runs].find(([, entry]) => entry.result.status !== 'running');
      if (!old) break;
      runs.delete(old[0]);
      sizes.delete(old[0]);
    }
    const controller = new AbortController();
    let runId: string | undefined;
    const promise = runScenario(req.body, {
      signal: controller.signal,
      onUpdate: (result) => {
        runId = result.id;
        runs.set(result.id, { result, controller });
        sizes.set(result.id, Buffer.byteLength(JSON.stringify(result)));
        trim();
      },
    });
    if (runId) res.status(202).json({ id: runId });
    else
      promise
        .catch(() => {})
        .finally(() => {
          if (!res.headersSent) res.status(400).json({ error: 'Nie można rozpocząć przebiegu.' });
        });
    promise.catch(() => {
      if (runId) {
        const entry = runs.get(runId);
        if (entry) {
          entry.result.status = 'failed';
          entry.result.finishedAt = new Date().toISOString();
          entry.result.error = 'Nieoczekiwany błąd runnera.';
        }
      }
    });
  });
  router.get('/:id', (req, res) => {
    const run = runs.get(String(req.params.id));
    if (!run) {
      res.status(404).json({ error: 'Nie znaleziono przebiegu (historia sesyjna).' });
      return;
    }
    res.json(run.result);
  });
  router.post('/:id/cancel', (req, res) => {
    const run = runs.get(String(req.params.id));
    if (!run) {
      res.status(404).json({ error: 'Nie znaleziono przebiegu.' });
      return;
    }
    run.controller.abort();
    res.json({ id: run.result.id, status: run.result.status });
  });
  return router;
}

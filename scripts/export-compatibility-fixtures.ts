import { createHash } from 'node:crypto';
import { mkdir, readFile, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import {
  createBlocksProject,
  createShopProject,
  createTicketsProject,
  createWorkflowProject,
} from '../shared/demos';
import { validateProject } from '../shared/graph';
import type { Project } from '../shared/model';

const ID_PATTERN = /^(node|order|scenario|project)-[0-9a-f]{8}$/;
type IdPrefix = 'node' | 'order' | 'scenario' | 'project';

const SCRIPT_DIR = path.dirname(fileURLToPath(import.meta.url));
const REPO_ROOT = path.resolve(SCRIPT_DIR, '..');
const FIXTURES_DIR = path.join(REPO_ROOT, 'fixtures', 'compatibility');

type FixtureSource = {
  slug: 'shop' | 'tickets' | 'blocks' | 'workflow';
  create: () => Project;
};

const FIXTURE_SOURCES: readonly FixtureSource[] = [
  { slug: 'shop', create: createShopProject },
  { slug: 'tickets', create: createTicketsProject },
  { slug: 'blocks', create: createBlocksProject },
  { slug: 'workflow', create: createWorkflowProject },
];

function stripSecretVariables(project: Project): Project {
  const cloned = structuredClone(project);
  const variables = { ...(cloned.environment?.variables ?? {}) };
  for (const secretName of cloned.environment?.secretNames ?? []) {
    delete variables[secretName];
  }
  cloned.environment = {
    ...cloned.environment,
    variables,
    secretNames: [...(cloned.environment?.secretNames ?? [])],
  };
  return cloned;
}

function collectIdMap(value: unknown): Map<string, string> {
  const counters: Record<IdPrefix, number> = {
    node: 0,
    order: 0,
    scenario: 0,
    project: 0,
  };
  const idMap = new Map<string, string>();

  function visit(current: unknown): void {
    if (Array.isArray(current)) {
      for (const item of current) {
        visit(item);
      }
      return;
    }

    if (current !== null && typeof current === 'object') {
      for (const [key, entryValue] of Object.entries(current as Record<string, unknown>)) {
        if (key === 'id' && typeof entryValue === 'string') {
          const match = ID_PATTERN.exec(entryValue);
          if (match && !idMap.has(entryValue)) {
            const prefix = match[1] as IdPrefix;
            counters[prefix] += 1;
            const deterministicId = `${prefix}-${String(counters[prefix]).padStart(8, '0')}`;
            idMap.set(entryValue, deterministicId);
          }
        }
        visit(entryValue);
      }
    }
  }

  visit(value);
  return idMap;
}

function replaceMappedStrings<T>(value: T, idMap: ReadonlyMap<string, string>): T {
  if (typeof value === 'string') {
    return (idMap.get(value) ?? value) as T;
  }

  if (Array.isArray(value)) {
    return value.map((item) => replaceMappedStrings(item, idMap)) as T;
  }

  if (value !== null && typeof value === 'object') {
    const out: Record<string, unknown> = {};
    for (const [key, entryValue] of Object.entries(value as Record<string, unknown>)) {
      out[key] = replaceMappedStrings(entryValue, idMap);
    }
    return out as T;
  }

  return value;
}

function normalizeProject(rawProject: Project, slug: string): Project {
  const initialErrors = validateProject(rawProject);
  if (initialErrors.length > 0) {
    throw new Error(`Invalid source project "${slug}": ${initialErrors.join('; ')}`);
  }

  const sanitized = stripSecretVariables(rawProject);
  const idMap = collectIdMap(sanitized);
  const normalized = replaceMappedStrings(sanitized, idMap);

  const normalizedErrors = validateProject(normalized);
  if (normalizedErrors.length > 0) {
    throw new Error(`Invalid normalized project "${slug}": ${normalizedErrors.join('; ')}`);
  }

  return normalized;
}

function toJsonText(value: unknown): string {
  return `${JSON.stringify(value, null, 2)}\n`;
}

function sha256Hex(content: string): string {
  return createHash('sha256').update(content, 'utf8').digest('hex');
}

function buildFixtureFiles(): Map<string, string> {
  const files = new Map<string, string>();
  const projects: Project[] = [];

  for (const source of FIXTURE_SOURCES) {
    const normalized = normalizeProject(source.create(), source.slug);
    projects.push(normalized);
    const relativePath = `projects/${source.slug}.json`;
    files.set(relativePath, toJsonText(normalized));
  }

  const workspace = {
    version: 1 as const,
    activeId: projects[0].id,
    projects,
  };
  files.set('workspace-v1.json', toJsonText(workspace));

  const manifestEntries = Array.from(files.entries())
    .sort(([pathA], [pathB]) => (pathA < pathB ? -1 : pathA > pathB ? 1 : 0))
    .map(([relativePath, content]) => ({
      path: relativePath,
      sha256: sha256Hex(content),
    }));

  const manifest = {
    schemaVersion: 1,
    projectFormatVersion: 1,
    files: manifestEntries,
  };
  files.set('manifest.json', toJsonText(manifest));

  return files;
}

function printUsage(stream: NodeJS.WriteStream): void {
  stream.write('Usage: node --import tsx scripts/export-compatibility-fixtures.ts [--check]\n');
}

async function writeFixtures(files: ReadonlyMap<string, string>): Promise<void> {
  for (const [relativePath, content] of files) {
    const targetPath = path.join(FIXTURES_DIR, ...relativePath.split('/'));
    await mkdir(path.dirname(targetPath), { recursive: true });
    await writeFile(targetPath, content, 'utf8');
  }
}

async function checkFixtures(files: ReadonlyMap<string, string>): Promise<boolean> {
  let allMatch = true;

  for (const [relativePath, expectedContent] of files) {
    const targetPath = path.join(FIXTURES_DIR, ...relativePath.split('/'));
    try {
      const actualBytes = await readFile(targetPath);
      const expectedBytes = Buffer.from(expectedContent, 'utf8');
      if (!actualBytes.equals(expectedBytes)) {
        process.stderr.write(`Fixture mismatch: ${relativePath}\n`);
        allMatch = false;
      }
    } catch {
      process.stderr.write(`Missing or unreadable fixture: ${relativePath}\n`);
      allMatch = false;
    }
  }

  return allMatch;
}

async function main(): Promise<void> {
  const args = process.argv.slice(2);
  let checkMode = false;

  if (args.length === 1 && args[0] === '--check') {
    checkMode = true;
  } else if (args.length > 0) {
    printUsage(process.stderr);
    process.exit(2);
  }

  const files = buildFixtureFiles();

  if (checkMode) {
    const ok = await checkFixtures(files);
    if (!ok) {
      process.exit(1);
    }
    return;
  }

  await writeFixtures(files);
}

main().catch((error: unknown) => {
  const message = error instanceof Error ? error.message : String(error);
  process.stderr.write(`${message}\n`);
  process.exit(1);
});

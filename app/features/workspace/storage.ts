import type { Project } from '../../../shared/model';
import { validateProject } from '../../../shared/graph';

export const STORAGE_KEY = 'apiforge.workspace.v1';
export interface Workspace {
  version: 1;
  activeId: string;
  projects: Project[];
}
function migrateDemoAddress(project: Project): Project {
  const legacy = /^http:\/\/(127\.0\.0\.1|localhost):3000\/demo\/(shop|support)\/?$/;
  if (!legacy.test(project.environment.baseUrl)) return project;
  return {
    ...project,
    environment: {
      ...project.environment,
      baseUrl: project.environment.baseUrl.replace(':3000/', ':4318/'),
    },
  };
}
export function parseProject(text: string): Project {
  const value = JSON.parse(text);
  const errors = validateProject(value);
  if (errors.length) throw new Error(errors.join(' '));
  return migrateDemoAddress(value);
}
export function parseWorkspace(text: string): Workspace {
  const value = JSON.parse(text) as Workspace;
  if (value?.version !== 1 || !Array.isArray(value.projects) || !value.projects.length)
    throw new Error('Nieobsługiwana wersja zapisu.');
  const ids = new Set<string>();
  for (const project of value.projects) {
    const errors = validateProject(project);
    if (errors.length) throw new Error(errors.join(' '));
    if (ids.has(project.id)) throw new Error('Powtórzone ID projektu.');
    ids.add(project.id);
  }
  if (!value.projects.some((p) => p.id === value.activeId))
    throw new Error('Brak aktywnego projektu.');
  return { ...value, projects: value.projects.map(migrateDemoAddress) };
}
export function publicProject(project: Project): Project {
  const clone = structuredClone(project);
  for (const name of clone.environment.secretNames) delete clone.environment.variables[name];
  return clone;
}
export function downloadJson(filename: string, value: unknown) {
  const url = URL.createObjectURL(
    new Blob([JSON.stringify(value, null, 2)], { type: 'application/json' }),
  );
  const link = document.createElement('a');
  link.href = url;
  link.download = filename;
  link.click();
  setTimeout(() => URL.revokeObjectURL(url), 1000);
}

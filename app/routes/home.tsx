import type { Route } from './+types/home';
import { Workspace } from '../features/workspace/Workspace';
export function meta({}: Route.MetaArgs) {
  return [
    { title: 'API Forge — wizualne testy regresji API' },
    {
      name: 'description',
      content: 'Projektuj testy API z własnych komponentów, połącz dane i sprawdzaj wyniki.',
    },
  ];
}
export default function Home() {
  return <Workspace />;
}

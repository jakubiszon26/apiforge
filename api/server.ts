import express from 'express';
import { fileURLToPath, pathToFileURL } from 'node:url';
import { existsSync } from 'node:fs';
import { createRunsRouter } from './routes/runs';
import { demoRouter } from './demo';

export function createApp() {
  const app = express();
  app.disable('x-powered-by');
  app.use((req, res, next) => {
    const host = req.headers.host ?? '';
    const localSocketHost =
      host === `127.0.0.1:${req.socket.localPort}` || host === `localhost:${req.socket.localPort}`;
    if (
      !localSocketHost &&
      !/^(127\.0\.0\.1|localhost)(:(4318|51(7[3-9]|8[0-9]|90)))?$/.test(host)
    ) {
      res.status(403).json({ error: 'Dozwolony jest wyłącznie lokalny host.' });
      return;
    }
    const origin = req.headers.origin;
    if (origin) {
      let allowed = false;
      try {
        const u = new URL(origin);
        allowed =
          u.protocol === 'http:' &&
          ['127.0.0.1', 'localhost'].includes(u.hostname) &&
          (u.port === String(req.socket.localPort) ||
            u.port === '4318' ||
            (+u.port >= 5173 && +u.port <= 5190)) &&
          u.origin === origin;
      } catch {
        /* Reject malformed origins. */
      }
      if (!allowed) {
        res.status(403).json({ error: 'Niedozwolone źródło żądania.' });
        return;
      }
      res.setHeader('Access-Control-Allow-Origin', origin);
      res.setHeader('Vary', 'Origin');
      res.setHeader('Access-Control-Allow-Methods', 'GET,POST,PUT,PATCH,DELETE,OPTIONS');
      res.setHeader('Access-Control-Allow-Headers', 'Content-Type');
    }
    if (req.headers['sec-fetch-site'] === 'cross-site' && !origin) {
      res.status(403).json({ error: 'Niedozwolone źródło żądania.' });
      return;
    }
    if (req.method === 'OPTIONS') {
      res.sendStatus(204);
      return;
    }
    const mutates = !['GET', 'HEAD'].includes(req.method);
    const hasBody =
      Number(req.headers['content-length'] ?? 0) > 0 || !!req.headers['transfer-encoding'];
    if (mutates && (req.path.startsWith('/api/') || hasBody) && !req.is('application/json')) {
      res.status(415).json({ error: 'Żądania API i żądania z body wymagają application/json.' });
      return;
    }
    res.setHeader('Cache-Control', 'no-store');
    next();
  });
  app.use(express.json({ limit: '4mb' }));
  app.get('/api/health', (_req, res) =>
    res.json({ status: 'ok', service: 'apiforge-runner', version: 1 }),
  );
  app.use('/api/runs', createRunsRouter());
  app.use('/demo', demoRouter);
  app.use('/api', (_req, res) =>
    res.status(404).json({ error: 'Nie znaleziono endpointa API Forge.' }),
  );
  const clientDir = fileURLToPath(new URL('../build/client/', import.meta.url));
  if (existsSync(`${clientDir}/index.html`)) {
    app.use(express.static(clientDir));
    app.get(/^(?!\/api(?:\/|$)|\/demo(?:\/|$)).*/, (_req, res) =>
      res.sendFile(`${clientDir}/index.html`),
    );
  }
  app.use(
    (error: any, _req: express.Request, res: express.Response, _next: express.NextFunction) => {
      res.status(error?.status === 413 ? 413 : 400).json({
        error:
          error?.status === 413 ? 'Żądanie przekracza limit 4 MiB.' : 'Nieprawidłowe żądanie JSON.',
      });
    },
  );
  return app;
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  const server = createApp().listen(4318, '127.0.0.1', () =>
    console.log('API Forge runner: http://127.0.0.1:4318'),
  );
  server.on('error', (error: NodeJS.ErrnoException) => {
    console.error(
      error.code === 'EADDRINUSE'
        ? 'Port 4318 jest zajęty. API Forge nie może uruchomić runnera.'
        : error.message,
    );
    process.exitCode = 1;
  });
}

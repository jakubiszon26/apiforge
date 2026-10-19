import { test, expect, type Page } from '@playwright/test';
import { createShopProject } from '../../shared/demos';
import type { RunResult } from '../../shared/model';

async function execute(page: Page): Promise<RunResult> {
  const started = page.waitForResponse(
    (r) => r.url().endsWith('/api/runs') && r.request().method() === 'POST',
  );
  await page.getByRole('button', { name: /^Uruchom test$/ }).click();
  const response = await started;
  expect(response.status()).toBe(202);
  const { id } = await response.json();
  let result: RunResult;
  await expect
    .poll(
      async () => {
        result = await (await page.request.get(`/api/runs/${id}`)).json();
        return result.status;
      },
      { timeout: 15000 },
    )
    .not.toBe('running');
  return result!;
}

test('real demo run shows results and actually removes the created cart', async ({ page }) => {
  const errors: string[] = [];
  page.on('pageerror', (e) => errors.push(e.message));
  await page.goto('/');
  await expect(
    page.getByRole('heading', { name: 'Promocja i cena w koszyku', exact: true }),
  ).toBeVisible();
  await expect(page.locator('.react-flow__node')).toHaveCount(6);
  await page.getByRole('button', { name: 'API', exact: true }).click();
  const result = await execute(page);
  expect(result.mode).toBe('live');
  expect(result.status).toBe('passed');
  expect(result.steps.filter((s) => s.phase === 'cleanup').map((s) => s.status)).toEqual([
    'passed',
    'passed',
  ]);
  const cart = result.steps.find((s) => s.nodeId === 'cart')!.outputs.cart as { id: string };
  expect((await page.request.get(`/demo/shop/carts/${cart.id}`)).status()).toBe(404);
  await expect(page.locator('.af-block .af-status').first()).toContainText('Zaliczony');
  await expect(page.getByText('Zakończony poprawnie', { exact: true })).toBeVisible();
  await page.getByRole('button', { name: /Ctrl \+ klik: zaznacz wiele/ }).click();
  await page.screenshot({ path: 'test-results/apiforge-desktop.png', fullPage: true });
  expect(errors).toEqual([]);
});

test('context action creates data binding and survives reload', async ({ page }) => {
  await page.goto('/');
  const product = page.locator('.react-flow__node[data-id="product"]');
  await expect(product).toBeVisible();
  await product.locator('.af-block-head').click({ button: 'right' });
  await expect(page.locator('.af-action-menu')).toBeVisible();
  await page
    .locator('.af-action-menu')
    .getByRole('button', { name: /Aktywna promocja/ })
    .click();
  await expect(page.locator('.react-flow__node')).toHaveCount(7);
  await expect
    .poll(async () =>
      page.evaluate(() => {
        const saved = JSON.parse(localStorage.getItem('apiforge.workspace.v1') ?? 'null');
        return saved?.projects.find((p: any) => p.id === saved.activeId)?.scenarios[0].graph.nodes
          .length;
      }),
    )
    .toBe(7);
  const bindings = await page.evaluate(() => {
    const w = JSON.parse(localStorage.getItem('apiforge.workspace.v1')!);
    return w.projects.find((p: any) => p.id === w.activeId).scenarios[0].graph.nodes.at(-1).inputs;
  });
  expect(bindings.product).toMatchObject({ kind: 'reference', nodeId: 'product', port: 'product' });
  await page.reload();
  await expect(page.locator('.react-flow__node')).toHaveCount(7);
});

test('nodes can be dragged and library blocks dropped onto the canvas', async ({ page }) => {
  const errors: string[] = [];
  page.on('pageerror', (e) => errors.push(e.message));
  page.on('console', (m) => {
    if (m.type() === 'warning' && m.text().includes('not initialized')) errors.push(m.text());
  });
  await page.goto('/');
  const product = page.locator('.react-flow__node[data-id="product"]');
  await expect(product).toBeVisible();
  const before = await product.getAttribute('style');
  const head = await product.locator('.af-block-head').boundingBox();
  expect(head).not.toBeNull();
  await page.mouse.move(head!.x + 25, head!.y + 20);
  await page.mouse.down();
  await page.mouse.move(head!.x + 105, head!.y + 65, { steps: 12 });
  await page.mouse.up();
  await expect.poll(() => product.getAttribute('style')).not.toBe(before);
  await expect
    .poll(() =>
      page.evaluate(() => {
        const w = JSON.parse(localStorage.getItem('apiforge.workspace.v1') ?? 'null');
        return w?.projects[0].scenarios[0].graph.nodes.find((n: any) => n.id === 'product').position
          .x;
      }),
    )
    .not.toBe(40);
  await page
    .locator('.af-library-request')
    .dragTo(page.locator('.react-flow__pane'), { targetPosition: { x: 180, y: 160 } });
  await expect(page.locator('.react-flow__node')).toHaveCount(7);
  await expect
    .poll(() =>
      page.evaluate(() => {
        const w = JSON.parse(localStorage.getItem('apiforge.workspace.v1') ?? 'null');
        return w?.projects[0].scenarios[0].graph.nodes.length;
      }),
    )
    .toBe(7);
  await page.reload();
  await expect(page.locator('.react-flow__node')).toHaveCount(7);
  expect(errors).toEqual([]);
});

test('run explains an HTML response and recovers on retry', async ({ page }) => {
  await page.goto('/');
  await page.route('**/api/runs', (route) =>
    route.fulfill({
      status: 404,
      contentType: 'text/html',
      body: '<!doctype html><h1>Other application</h1>',
    }),
  );
  await page.getByRole('button', { name: 'Uruchom test', exact: true }).click();
  await expect(page.getByRole('status')).toContainText('odpowiedź inną niż JSON');
  await expect(page.getByRole('button', { name: 'Uruchom test', exact: true })).toBeEnabled();
  await page.unroute('**/api/runs');
  expect((await execute(page)).status).toBe('passed');
});

test('author a request, save a reusable component and assert its real response', async ({
  page,
}) => {
  await page.goto('/');
  await page.getByTitle('Nowy scenariusz', { exact: true }).click();
  await page.locator('.af-new-item').getByLabel('Nazwa').fill('Własny endpoint');
  await page.locator('.af-new-item').getByRole('button', { name: 'Utwórz' }).click();
  await page.locator('.af-library-request').click();
  await page.getByLabel('Nazwa klocka', { exact: true }).fill('Pobierz dane');
  await page.getByLabel('URL lub ścieżka', { exact: true }).fill('/products/:code');
  await page.getByRole('textbox', { name: 'Nazwa nowego argumentu', exact: true }).fill('code');
  await page.getByRole('textbox', { name: 'Nazwa nowego argumentu', exact: true }).press('Enter');
  await page.locator('.af-inline-input[aria-label="code"]').fill('123');
  await page.locator('.af-inline-input[aria-label="code"]').press('Tab');
  await page.locator('.af-save-component').click();
  const dialog = page.getByRole('dialog');
  await dialog.getByLabel('Nazwa', { exact: true }).fill('Mój komponent');
  await dialog.getByRole('button', { name: 'Zapisz komponent', exact: true }).click();
  await expect(dialog).not.toBeVisible();
  await page.locator('.af-library-component').filter({ hasText: 'Mój komponent' }).click();
  await expect(page.locator('.react-flow__node')).toHaveCount(3);
  const componentId = await page
    .locator('.react-flow__node')
    .filter({ hasText: 'Mój komponent' })
    .getAttribute('data-id');
  await page.locator('.af-kind-trigger .af-block-head').click();
  await page
    .getByRole('combobox', { name: 'Następny krok: dalej', exact: true })
    .selectOption(componentId!);
  await page.getByRole('button', { name: 'API', exact: true }).click();
  const result = await execute(page);
  expect(result.status).toBe('passed');
  const instance = result.steps.find((s) => s.name === 'Mój komponent')!;
  expect(instance.children?.[0].request?.url).toContain('/products/123');
  await expect(page.getByText('Zakończony poprawnie', { exact: true })).toBeVisible();
  await page.locator('.af-run-step-row').filter({ hasText: 'Mój komponent' }).click();
  await page.locator('.af-step-details').getByTitle('Sprawdź id', { exact: true }).click();
  await expect(page.locator('.react-flow__node')).toHaveCount(4);
  const asserted = await execute(page);
  expect(asserted.status).toBe('passed');
  expect(asserted.steps.find((s) => s.kind === 'assert')?.actual).toBe(123);
});

test('one comparison handles values and types through its arguments', async ({ page }) => {
  await page.goto('/');
  await page.locator('.react-flow__node[data-id="product"] .af-block-head').click();
  const inspector = page.getByRole('complementary', { name: 'Konfiguracja klocka' });
  await inspector.getByRole('button', { name: 'Dane wyjściowe', exact: true }).click();
  await expect(inspector.getByRole('button', { name: 'Sprawdź typ', exact: true })).toHaveCount(0);
  await inspector.getByTitle('Sprawdź promotion.active', { exact: true }).click();
  await expect(inspector.getByLabel('Sprawdzenie', { exact: true })).toHaveValue('equals');
  await inspector.getByLabel('Sprawdzenie', { exact: true }).selectOption('type');
  await inspector
    .getByRole('combobox', { name: 'Oczekiwany typ', exact: true })
    .selectOption('boolean');
  await expect(
    inspector.getByRole('combobox', { name: 'Oczekiwany typ', exact: true }),
  ).toHaveValue('boolean');
  await expect(inspector.getByLabel('Sprawdzenie', { exact: true })).toHaveValue('type');
  const result = await execute(page);
  expect(result.steps.find((step) => step.name === 'Sprawdź promotion.active')).toMatchObject({
    status: 'passed',
    expected: 'boolean',
    actual: true,
  });
  await inspector
    .getByRole('combobox', { name: 'Oczekiwany typ', exact: true })
    .selectOption('string');
  const mismatch = await execute(page);
  expect(mismatch.steps.find((step) => step.name === 'Sprawdź promotion.active')?.status).toBe(
    'failed',
  );
  await inspector
    .getByRole('combobox', { name: 'Oczekiwany typ', exact: true })
    .selectOption('boolean');
  await execute(page);
  await page.locator('.af-run-step-row').filter({ hasText: 'Sprawdź promotion.active' }).click();
  await expect(
    page.locator('.af-step-details').getByRole('button', { name: 'Sprawdź typ', exact: true }),
  ).toHaveCount(0);
  await page.locator('.af-step-details').getByTitle('Sprawdź value', { exact: true }).click();
  await expect(inspector.getByLabel('Sprawdzenie', { exact: true })).toHaveValue('equals');
  await inspector.getByLabel('Sprawdzenie', { exact: true }).selectOption('type');
  // Expected type remains an argument, including parameter bindings.
  await inspector
    .getByRole('combobox', { name: 'Źródło expected', exact: true })
    .selectOption('parameter');
  await inspector
    .getByRole('combobox', { name: 'Nazwa expected', exact: true })
    .fill('expectedType');
  await inspector.getByLabel('Sprawdzenie', { exact: true }).selectOption('equals');
  await expect(
    inspector.getByRole('combobox', { name: 'Źródło expected', exact: true }),
  ).toHaveValue('parameter');
  await inspector.getByLabel('Sprawdzenie', { exact: true }).selectOption('type');
  await expect(
    inspector.getByRole('combobox', { name: 'Źródło expected', exact: true }),
  ).toHaveValue('parameter');
  await inspector
    .getByRole('combobox', { name: 'Źródło expected', exact: true })
    .selectOption('literal');
  await inspector
    .getByRole('combobox', { name: 'Oczekiwany typ', exact: true })
    .selectOption('boolean');
  expect((await execute(page)).status).toBe('passed');
  await page.screenshot({ path: 'test-results/apiforge-type-assertion.png', fullPage: true });
});

test('run panel resizes by dragging and keyboard and keeps its height when collapsed', async ({
  page,
}) => {
  await page.goto('/');
  const divider = page.getByRole('separator', { name: 'Wysokość przebiegu testu' });
  const report = page.locator('.af-resizable-report');
  const before = (await report.boundingBox())!.height;
  const editorBefore = (await page.locator('.af-editing-area').boundingBox())!.height;
  const handle = (await divider.boundingBox())!;
  await page.mouse.move(handle.x + handle.width / 2, handle.y + handle.height / 2);
  await page.mouse.down();
  await page.mouse.move(handle.x + handle.width / 2, handle.y - 100, { steps: 10 });
  await page.mouse.up();
  await expect.poll(async () => (await report.boundingBox())!.height).toBeGreaterThan(before + 90);
  expect((await page.locator('.af-editing-area').boundingBox())!.height).toBeLessThan(
    editorBefore - 90,
  );
  await divider.focus();
  await page.keyboard.press('Home');
  await expect(divider).toHaveAttribute('aria-valuenow', '160');
  await page.keyboard.press('ArrowUp');
  await expect(divider).toHaveAttribute('aria-valuenow', '192');
  await page.getByRole('button', { name: 'Ukryj przebieg', exact: true }).click();
  await expect(report).toBeHidden();
  await page.getByRole('button', { name: 'Pokaż przebieg', exact: true }).click();
  await expect(report).toBeVisible();
  await expect(divider).toHaveAttribute('aria-valuenow', '192');
});

test('nested operator blocks run against API, edit, persist and accept a dropped operator', async ({
  page,
}) => {
  const errors: string[] = [];
  page.on('pageerror', (error) => errors.push(error.message));
  await page.goto('/');
  await page.getByRole('button', { name: 'Otwórz przykład blokowy', exact: true }).click();
  await expect(
    page.getByRole('heading', { name: 'Test z zagnieżdżonych bloków', exact: true }),
  ).toBeVisible();
  await page.getByRole('button', { name: 'API', exact: true }).click();
  const first = await execute(page);
  expect(first.mode).toBe('live');
  expect(first.status).toBe('passed');
  expect(first.steps.find((step) => step.nodeId === 'name-check')?.actual).toBe(true);
  await page.locator('.react-flow__node[data-id="name-check"] .af-block-head').click();
  const inspector = page.getByRole('complementary', { name: 'Konfiguracja klocka' });
  const equality = inspector.locator('.af-expression[data-operator="equals"]');
  await equality.getByRole('textbox', { name: 'Wartość right', exact: true }).fill('number');
  const failed = await execute(page);
  expect(failed.steps.find((step) => step.nodeId === 'name-check')).toMatchObject({
    status: 'failed',
    actual: false,
    expected: true,
  });
  await equality.getByRole('textbox', { name: 'Wartość right', exact: true }).fill('string');
  await page.reload();
  await page.locator('.react-flow__node[data-id="name-check"] .af-block-head').click();
  await expect(
    inspector
      .locator('.af-expression[data-operator="equals"]')
      .getByRole('textbox', { name: 'Wartość right', exact: true }),
  ).toHaveValue('string');
  const logic = page
    .locator('.af-operator-palette details')
    .filter({ has: page.locator('summary', { hasText: 'Logika' }) });
  await logic.locator('summary').click();
  const slot = inspector.locator('.af-port').first().locator('.af-binding-drop-target').first();
  await page
    .getByRole('button', { name: /^Nie A/ })
    .dragTo(slot, { targetPosition: { x: 20, y: 20 } });
  await expect(inspector.locator('.af-expression[data-operator="not"]')).toHaveCount(1);
  expect((await execute(page)).steps.find((step) => step.nodeId === 'name-check')).toMatchObject({
    status: 'failed',
    actual: false,
  });
  await inspector
    .locator('.af-expression[data-operator="not"]')
    .locator(':scope > header')
    .getByRole('button', { name: 'Usuń operator', exact: true })
    .click();
  await expect(inspector.locator('.af-expression[data-operator="not"]')).toHaveCount(0);
  expect((await execute(page)).status).toBe('passed');
  await page.getByRole('button', { name: /^Ukryj przebieg/ }).click();
  await inspector.locator('.af-inspector-content').evaluate((element) => {
    element.scrollTop = 230;
  });
  await page.screenshot({ path: 'test-results/apiforge-nested-blocks.png', fullPage: true });
  expect(errors).toEqual([]);
});

test('comparison chooses searchable fields from the last HTTP body without a sample', async ({
  page,
}) => {
  await page.goto('/');
  await page.getByTitle('Nowy scenariusz', { exact: true }).click();
  await page.locator('.af-new-item').getByLabel('Nazwa').fill('Wybieranie pól odpowiedzi');
  await page.locator('.af-new-item').getByRole('button', { name: 'Utwórz' }).click();
  await page.locator('.af-library-request').click();
  await page.getByLabel('URL lub ścieżka', { exact: true }).fill('/products/123');
  await page.getByRole('button', { name: 'API', exact: true }).click();
  expect((await execute(page)).status).toBe('passed');
  await page.locator('.af-library-assert').click();
  const inspector = page.getByRole('complementary', { name: 'Konfiguracja klocka' });
  await inspector
    .getByRole('combobox', { name: 'Źródło actual', exact: true })
    .selectOption('reference');
  await inspector.getByRole('button', { name: 'Wybierz pole z odpowiedzi', exact: true }).click();
  const dialog = page.getByRole('dialog', { name: 'Wybierz pole odpowiedzi', exact: true });
  await expect(dialog).toContainText('Ostatnia odpowiedź · API');
  await page.screenshot({
    path: 'test-results/apiforge-response-field-picker.png',
    fullPage: true,
  });
  await dialog.getByRole('textbox', { name: 'Szukaj pola', exact: true }).fill('promotion.price');
  await expect(dialog.getByTitle('Wybierz promotion.price', { exact: true })).toContainText('80');
  await dialog.getByTitle('Wybierz promotion.price', { exact: true }).click();
  await expect(dialog).toBeHidden();
  await expect(inspector.getByRole('textbox', { name: 'Ścieżka actual', exact: true })).toHaveValue(
    'promotion.price',
  );
  await inspector.getByRole('combobox', { name: 'Typ danych', exact: true }).selectOption('number');
  await inspector.getByRole('spinbutton', { name: 'Wartość expected', exact: true }).fill('80');
  expect((await execute(page)).status).toBe('passed');
  // Editing the assertion makes the run historical; its response is still useful for choosing another field.
  await inspector.getByRole('button', { name: 'Wybierz pole z odpowiedzi', exact: true }).click();
  await dialog.getByRole('textbox', { name: 'Szukaj pola', exact: true }).fill('name');
  await dialog.getByTitle('Wybierz name', { exact: true }).click();
  await expect(inspector.getByRole('textbox', { name: 'Ścieżka actual', exact: true })).toHaveValue(
    'name',
  );
  await inspector.getByRole('button', { name: 'Wybierz pole z odpowiedzi', exact: true }).click();
  await dialog.getByTitle('Wybierz całe body', { exact: true }).click();
  await expect(inspector.getByRole('textbox', { name: 'Ścieżka actual', exact: true })).toHaveValue(
    '',
  );
});

test('variable blocks drop into URL, query, headers and comparison inputs without copying values', async ({
  page,
}) => {
  await page.goto('/');
  await page.getByTitle('Nowy scenariusz', { exact: true }).click();
  await page.locator('.af-new-item').getByLabel('Nazwa').fill('Bloczki zmiennych');
  await page.locator('.af-new-item').getByRole('button', { name: 'Utwórz' }).click();
  await page.getByRole('button', { name: 'Dodaj zmienną', exact: true }).click();
  const inspector = page.getByRole('complementary', { name: 'Konfiguracja klocka' });
  await inspector.getByLabel('Nazwa klocka', { exact: true }).fill('productCode');
  await inspector.getByRole('combobox', { name: 'Typ danych', exact: true }).selectOption('number');
  await inspector.getByRole('spinbutton', { name: 'Wartość value', exact: true }).fill('123');
  const variable = page.getByRole('button', { name: 'Zmienna productCode', exact: true });
  await page.locator('.af-library-request').click();
  const url = inspector.getByLabel('URL lub ścieżka', { exact: true });
  await url.fill('/products/');
  await url.press('End');
  await variable.dragTo(url);
  await expect(url).toHaveValue('/products/{{productCode}}');
  const queryName = inspector.getByRole('textbox', {
    name: 'Nowa nazwa: Parametry query',
    exact: true,
  });
  await queryName.fill('code');
  await queryName.locator('..').getByRole('button', { name: 'Dodaj', exact: true }).click();
  await variable.dragTo(
    inspector.getByRole('textbox', { name: 'Parametry query: code', exact: true }),
  );
  const headerName = inspector.getByRole('textbox', { name: 'Nowa nazwa: Nagłówki', exact: true });
  await headerName.fill('X-Code');
  await headerName.locator('..').getByRole('button', { name: 'Dodaj', exact: true }).click();
  await variable.dragTo(inspector.getByRole('textbox', { name: 'Nagłówki: X-Code', exact: true }));
  await page.locator('.af-library-assert').click();
  const actual = inspector.locator('.af-port').first();
  await actual.getByRole('combobox', { name: 'Typ danych', exact: true }).selectOption('string');
  await variable.dragTo(actual.getByRole('textbox', { name: 'Wartość actual', exact: true }));
  await expect(actual.locator('.af-bound-variable')).toContainText('productCode');
  const expected = inspector.locator('.af-port').nth(1);
  await expected.getByRole('combobox', { name: 'Typ danych', exact: true }).selectOption('number');
  await expected.getByRole('spinbutton', { name: 'Wartość expected', exact: true }).fill('123');
  await page.getByRole('button', { name: 'API', exact: true }).click();
  const first = await execute(page);
  expect(first.status).toBe('passed');
  const request = first.steps.find((step) => step.kind === 'request')!;
  expect(request.request?.url).toContain('/products/123?code=123');
  expect(request.request?.headers['X-Code']).toBe('123');
  await page.screenshot({ path: 'test-results/apiforge-variable-blocks.png', fullPage: true });
  await variable.click();
  await inspector.getByRole('spinbutton', { name: 'Wartość value', exact: true }).fill('456');
  const second = await execute(page);
  expect(second.steps.find((step) => step.kind === 'assert')).toMatchObject({
    status: 'failed',
    actual: 456,
    expected: 123,
  });
  expect(second.steps.find((step) => step.kind === 'request')?.request?.url).toContain(
    '/products/456?code=456',
  );
  await inspector.getByRole('spinbutton', { name: 'Wartość value', exact: true }).fill('123');
  await variable.dragTo(inspector.getByRole('spinbutton', { name: 'Wartość value', exact: true }));
  await expect(inspector.getByText(/cykl/)).toBeVisible();
  expect((await execute(page)).status).toBe('passed');
  await page.reload();
  await page.getByRole('button', { name: 'Bloczki zmiennych', exact: true }).click();
  await page.getByRole('button', { name: 'API', exact: true }).click();
  await expect(variable).toBeVisible();
  expect((await execute(page)).status).toBe('passed');
});

test('capture a response field in a variable and use it in a comparison', async ({ page }) => {
  await page.goto('/');
  await page.getByTitle('Nowy scenariusz', { exact: true }).click();
  await page.locator('.af-new-item').getByLabel('Nazwa').fill('Zmienne z odpowiedzi');
  await page.locator('.af-new-item').getByRole('button', { name: 'Utwórz' }).click();
  await page.locator('.af-library-request').click();
  const inspector = page.getByRole('complementary', { name: 'Konfiguracja klocka' });
  await inspector.getByLabel('Nazwa klocka', { exact: true }).fill('Odczytaj produkt');
  await inspector.getByLabel('URL lub ścieżka', { exact: true }).fill('/products/123');
  await page.getByRole('button', { name: 'API', exact: true }).click();
  expect((await execute(page)).status).toBe('passed');
  await page
    .locator('.af-step-details')
    .getByTitle('Przypisz price do zmiennej', { exact: true })
    .click();
  const capture = page.getByRole('dialog', { name: 'Przypisz pole do zmiennej', exact: true });
  await capture.getByLabel('Nazwa zmiennej', { exact: true }).fill('responsePrice');
  await capture.getByRole('button', { name: 'Przypisz zmienną', exact: true }).click();
  await expect(capture).toBeHidden();
  const variable = page.getByRole('button', { name: 'Zmienna responsePrice', exact: true });
  await expect(variable).toBeVisible();
  await page.locator('.af-library-assert').click();
  const actual = inspector.locator('.af-port').first();
  await actual.getByRole('combobox', { name: 'Typ danych', exact: true }).selectOption('string');
  await variable.dragTo(actual.getByRole('textbox', { name: 'Wartość actual', exact: true }));
  const expected = inspector.locator('.af-port').nth(1);
  await expected.getByRole('combobox', { name: 'Typ danych', exact: true }).selectOption('number');
  await expected.getByRole('spinbutton', { name: 'Wartość expected', exact: true }).fill('100');
  const first = await execute(page);
  expect(first.status).toBe('passed');
  expect(first.steps.find((step) => step.name === 'responsePrice')?.outputs.value).toBe(100);
  expect(first.steps.find((step) => step.kind === 'assert')?.actual).toBe(100);
  await variable.click();
  await inspector
    .getByRole('button', { name: 'Wybierz wartość z odpowiedzi', exact: true })
    .click();
  const fields = page.getByRole('dialog', { name: 'Wybierz pole odpowiedzi', exact: true });
  await expect(
    fields.getByRole('combobox', { name: 'Źródło odpowiedzi', exact: true }),
  ).toContainText('Odczytaj produkt');
  await fields.getByRole('textbox', { name: 'Szukaj pola', exact: true }).fill('promotion.price');
  await fields.getByTitle('Wybierz promotion.price', { exact: true }).click();
  const changed = await execute(page);
  expect(changed.steps.find((step) => step.name === 'responsePrice')?.outputs.value).toBe(80);
  expect(changed.steps.find((step) => step.kind === 'assert')).toMatchObject({
    status: 'failed',
    actual: 80,
    expected: 100,
  });
  await page.locator('.af-run-step-row').filter({ hasText: 'Porównanie' }).click();
  await inspector.getByRole('spinbutton', { name: 'Wartość expected', exact: true }).fill('80');
  expect((await execute(page)).status).toBe('passed');
  await page.screenshot({ path: 'test-results/apiforge-response-variable.png', fullPage: true });
});

test('assign a response field to an existing variable and drag a different field onto its block', async ({
  page,
}) => {
  await page.goto('/');
  await page.getByRole('button', { name: 'Otwórz przykład blokowy', exact: true }).click();
  await page.getByRole('button', { name: 'API', exact: true }).click();
  expect((await execute(page)).status).toBe('passed');
  await page.getByRole('button', { name: 'Dodaj zmienną', exact: true }).click();
  const inspector = page.getByRole('complementary', { name: 'Konfiguracja klocka' });
  await inspector.getByLabel('Nazwa klocka', { exact: true }).fill('savedField');
  await page.getByRole('button', { name: /Ctrl \+ klik: zaznacz wiele/ }).click();
  await page.locator('.react-flow__node[data-id="response"] .af-block-head').click();
  await inspector.getByRole('button', { name: 'Dane wyjściowe', exact: true }).click();
  await inspector.getByTitle('Przypisz promotion.price do zmiennej', { exact: true }).click();
  const capture = page.getByRole('dialog', { name: 'Przypisz pole do zmiennej', exact: true });
  await capture
    .getByRole('combobox', { name: 'Zmienna docelowa', exact: true })
    .selectOption({ label: 'savedField' });
  await capture.getByRole('button', { name: 'Przypisz zmienną', exact: true }).click();
  await expect(page.locator('.react-flow__node')).toHaveCount(4);
  expect(
    (await execute(page)).steps.find((step) => step.name === 'savedField')?.outputs.value,
  ).toBe(80);
  await page.locator('.react-flow__node[data-id="response"] .af-block-head').click();
  await inspector.getByRole('button', { name: 'Dane wyjściowe', exact: true }).click();
  const target = page
    .locator('.react-flow__node')
    .filter({ has: page.locator('.af-block-head strong', { hasText: 'savedField' }) })
    .locator('.af-input-port')
    .first();
  await inspector.locator('.af-json-row > code[title="price"]').dragTo(target);
  const result = await execute(page);
  expect(result.steps.find((step) => step.name === 'savedField')?.outputs.value).toBe(100);
  expect(result.status).toBe('passed');
  await page.reload();
  await expect(page.getByRole('button', { name: 'Zmienna savedField', exact: true })).toBeVisible();
  await page.getByRole('button', { name: 'API', exact: true }).click();
  expect(
    (await execute(page)).steps.find((step) => step.name === 'savedField')?.outputs.value,
  ).toBe(100);
});

test('workflow Start, IF branches and ordering survive editing and reload', async ({ page }) => {
  const errors: string[] = [];
  page.on('pageerror', (error) => errors.push(error.message));
  await page.goto('/');
  await page.getByRole('button', { name: 'Przykład: kolejność i IF', exact: true }).click();
  await expect(page.locator('.af-kind-trigger')).toHaveCount(1);
  await expect(page.locator('.af-kind-if')).toHaveCount(1);
  await page.getByRole('button', { name: 'API', exact: true }).click();
  const run = await execute(page);
  expect(run.status).toBe('passed');
  expect(run.steps.filter((step) => step.request).map((step) => step.nodeId)).toEqual([
    'flow-prices',
    'flow-cart',
    'flow-add',
    'flow-cleanup',
  ]);
  expect(run.steps.find((step) => step.nodeId === 'flow-else')?.status).toBe('skipped');
  await page.getByRole('button', { name: /^Ukryj przebieg/ }).click();
  await page.locator('.react-flow__node[data-id="flow-if"] .af-block-head').click();
  await expect(
    page.getByRole('combobox', { name: 'Następny krok: tak', exact: true }),
  ).toBeVisible();
  await expect(
    page.getByRole('combobox', { name: 'Następny krok: nie', exact: true }),
  ).toBeVisible();
  // Wire an existing block through the accessible selector, then verify persistence.
  await page
    .getByRole('combobox', { name: 'Następny krok: nie', exact: true })
    .selectOption('flow-check');
  await expect
    .poll(async () =>
      page.evaluate(() => {
        const saved = JSON.parse(localStorage.getItem('apiforge.workspace.v1')!);
        return saved.projects
          .find((p: any) => p.id === 'workflow-poc')
          .scenarios[0].graph.edges.some(
            (e: any) => e.source === 'flow-if' && e.target === 'flow-check' && e.branch === 'false',
          );
      }),
    )
    .toBe(true);
  await page.screenshot({ path: 'test-results/apiforge-workflow.png', fullPage: true });
  await page.reload();
  await page.getByRole('button', { name: 'API', exact: true }).click();
  const repeated = await execute(page);
  expect(repeated.status).toBe('passed');
  expect(repeated.steps.filter((step) => step.request).map((step) => step.nodeId)).toEqual([
    'flow-prices',
    'flow-cart',
    'flow-add',
    'flow-cleanup',
  ]);
  await page.getByRole('button', { name: /^Ukryj przebieg/ }).click();
  await page.locator('.react-flow__node[data-id="flow-prices"] .af-block-head').click();
  await page.getByLabel('URL lub ścieżka', { exact: true }).fill('/products/999');
  const failed = await execute(page);
  expect(failed.steps.find((step) => step.nodeId === 'flow-if')).toMatchObject({
    status: 'passed',
    outputs: { passed: false },
  });
  expect(failed.steps.find((step) => step.nodeId === 'flow-else')?.status).toBe('failed');
  expect(failed.steps.filter((step) => step.request).map((step) => step.nodeId)).toEqual([
    'flow-prices',
  ]);
  expect(errors).toEqual([]);
});

test('copy/paste keeps component configuration, supports repeated paste and undo, and leaves text shortcuts alone', async ({
  page,
}) => {
  await page.goto('/');
  const original = createShopProject().scenarios[0].graph.nodes.find(
    (node) => node.id === 'product',
  )!;
  const product = page.locator('.react-flow__node[data-id="product"]');
  await product.locator('.af-block-head').click();
  const modifier = process.platform === 'darwin' ? 'Meta' : 'Control';
  await page.keyboard.press(`${modifier}+c`);
  const name = page.getByLabel('Nazwa klocka', { exact: true });
  await name.fill('Zmieniony oryginał');
  await product.locator('.af-block-head').click();
  await page.keyboard.press(`${modifier}+v`);
  await expect(page.locator('.react-flow__node')).toHaveCount(7);
  const copiedName = `${original.name} — kopia`;
  await expect(name).toHaveValue(copiedName);
  await page.keyboard.press(`${modifier}+v`);
  await expect(page.locator('.react-flow__node')).toHaveCount(8);
  await expect
    .poll(async () =>
      page.evaluate((name) => {
        const saved = JSON.parse(localStorage.getItem('apiforge.workspace.v1') ?? 'null');
        return saved?.projects[0].scenarios[0].graph.nodes.filter((node: any) => node.name === name)
          .length;
      }, copiedName),
    )
    .toBe(2);
  const copies = await page.evaluate((name) => {
    const saved = JSON.parse(localStorage.getItem('apiforge.workspace.v1')!);
    return saved.projects[0].scenarios[0].graph.nodes.filter((node: any) => node.name === name);
  }, copiedName);
  expect(copies[0].config).toEqual(original.config);
  expect(copies[0].inputs).toEqual(original.inputs);
  expect(copies[1].config).toEqual(original.config);
  expect(copies[1].id).not.toBe(copies[0].id);
  expect(copies[1].position).not.toEqual(copies[0].position);
  await page.keyboard.press(`${modifier}+z`);
  await expect(page.locator('.react-flow__node')).toHaveCount(7);
  await page.keyboard.press(`${modifier}+Shift+z`);
  await expect(page.locator('.react-flow__node')).toHaveCount(8);
  // Native text copy must not overwrite the board clipboard or duplicate nodes.
  await name.focus();
  await name.selectText();
  await page.keyboard.press(`${modifier}+c`);
  await page.keyboard.press(`${modifier}+v`);
  await expect(page.locator('.react-flow__node')).toHaveCount(8);
  await product.locator('.af-block-head').click();
  await page.getByTitle('Wklej klocki z konfiguracją (Ctrl/Cmd+V)', { exact: true }).click();
  await expect(page.locator('.react-flow__node')).toHaveCount(9);
  await page.getByTitle('Duplikuj zaznaczenie (Ctrl/Cmd+D)', { exact: true }).click();
  await expect(page.locator('.react-flow__node')).toHaveCount(10);
  await page.reload();
  await expect(page.locator('.react-flow__node')).toHaveCount(10);
});

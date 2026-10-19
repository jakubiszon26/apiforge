import type { Binding } from './model';
import { createNode } from './graph';
import type { Operator } from './operators';
import { literal, reference, uid } from './model';
import type { ComponentDefinition, Graph, Json, Project, Scenario, TestNode } from './model';

const empty = (): Graph => ({ nodes: [], edges: [] });
const param = (name: string) => ({ kind: 'parameter' as const, name });
const product: Json = {
  id: 123,
  name: 'Kawa ziarnista',
  price: 100,
  promotion: { type: 'fixed_price', active: true, price: 80, inna_nazwa: 'Oferta jesienna' },
};
const request = (
  id: string,
  name: string,
  url: string,
  inputs: TestNode['inputs'],
  sampleResponse: Json,
  method = 'GET',
  x = 80,
  y = 80,
): TestNode => ({
  id,
  kind: 'request',
  name,
  position: { x, y },
  inputs,
  config: {
    method,
    url,
    headers: {},
    query: {},
    timeoutMs: 10000,
    inputPorts: Object.keys(inputs)
      .filter((k) => k !== 'body')
      .map((name) => ({ name, type: 'any', exposed: true, required: true })),
    sampleResponse,
  },
});

export function createShopProject(): Project {
  const productComponent: ComponentDefinition = {
    id: 'get-product',
    revision: 1,
    name: 'Produkt',
    description: 'Pobierz produkt po ID wraz z promocją.',
    inputs: [
      {
        name: 'id',
        label: 'ID produktu',
        type: 'number',
        defaultValue: 123,
        required: true,
        exposed: true,
      },
    ],
    outputs: [
      {
        name: 'product',
        label: 'Produkt',
        type: 'object',
        contractId: 'Product',
        binding: reference('fetch', 'body'),
      },
    ],
    graph: {
      nodes: [request('fetch', 'Pobierz produkt', '/products/:id', { id: param('id') }, product)],
      edges: [],
    },
  };
  const activeComponent: ComponentDefinition = {
    id: 'active-promotion',
    revision: 1,
    name: 'Aktywna promocja',
    description: 'Sprawdź, czy produkt ma aktywną promocję.',
    inputs: [
      { name: 'product', label: 'Produkt', type: 'object', contractId: 'Product', required: true },
    ],
    outputs: [
      { name: 'product', type: 'object', contractId: 'Product', binding: param('product') },
    ],
    action: { input: 'product', type: 'object', contractId: 'Product' },
    graph: {
      nodes: [
        {
          id: 'active',
          kind: 'select',
          name: 'Aktywność promocji',
          position: { x: 80, y: 80 },
          inputs: { value: param('product') },
          config: { path: 'promotion.active' },
        },
        {
          id: 'check',
          kind: 'condition',
          name: 'Promocja musi być aktywna',
          position: { x: 400, y: 80 },
          inputs: { actual: reference('active', 'value'), expected: literal(true) },
          config: { predicate: 'equals' },
        },
      ],
      edges: [],
    },
  };
  const basketComponent: ComponentDefinition = {
    id: 'product-cart',
    revision: 1,
    name: 'Dodaj do koszyka',
    description: 'Utwórz koszyk z produktem i pobierz jego wyliczoną cenę.',
    inputs: [
      { name: 'product', type: 'object', contractId: 'Product', required: true },
      {
        name: 'quantity',
        label: 'Ilość',
        type: 'number',
        defaultValue: 1,
        exposed: true,
        required: true,
      },
    ],
    outputs: [
      {
        name: 'cart',
        label: 'Koszyk',
        type: 'object',
        contractId: 'Cart',
        binding: reference('read', 'body'),
      },
    ],
    action: { input: 'product', type: 'object', contractId: 'Product' },
    graph: {
      nodes: [
        {
          id: 'id',
          kind: 'select',
          name: 'ID produktu',
          position: { x: 50, y: 90 },
          inputs: { value: param('product') },
          config: { path: 'id' },
        },
        {
          id: 'body',
          kind: 'build',
          name: 'Pozycja koszyka',
          position: { x: 350, y: 90 },
          inputs: { productId: reference('id', 'value'), quantity: param('quantity') },
          config: { buildType: 'object' },
        },
        request(
          'create',
          'Utwórz koszyk',
          '/carts',
          { body: reference('body', 'value') },
          { id: 'sample-cart', productId: 123, quantity: 1, total: 80 },
          'POST',
          650,
          90,
        ),
        request(
          'read',
          'Pobierz koszyk',
          '/carts/:id',
          { id: reference('create', 'body', 'id') },
          { id: 'sample-cart', productId: 123, quantity: 1, total: 80 },
          'GET',
          950,
          90,
        ),
      ],
      edges: [],
    },
  };
  const nodes: TestNode[] = [
    {
      id: 'product',
      kind: 'component',
      name: 'Produkt',
      position: { x: 40, y: 160 },
      inputs: { id: literal(123) },
      config: { componentId: 'get-product', revision: 1 },
    },
    {
      id: 'active',
      kind: 'component',
      name: 'Aktywna promocja',
      position: { x: 360, y: 30 },
      inputs: { product: reference('product', 'product') },
      config: { componentId: 'active-promotion', revision: 1 },
    },
    {
      id: 'expected',
      kind: 'formula',
      name: 'Oczekiwana cena',
      position: { x: 360, y: 290 },
      inputs: { left: literal(80), right: literal(1) },
      config: { operation: 'multiply' },
    },
    {
      id: 'cart',
      kind: 'component',
      name: 'Dodaj do koszyka',
      position: { x: 680, y: 30 },
      inputs: { product: reference('active', 'product'), quantity: literal(1) },
      config: { componentId: 'product-cart', revision: 1 },
    },
    {
      id: 'price',
      kind: 'assert',
      name: 'Cena po promocji',
      position: { x: 1000, y: 160 },
      inputs: {
        actual: reference('cart', 'cart', 'total'),
        expected: reference('expected', 'value'),
      },
      config: { predicate: 'approximately', tolerance: 0.005 },
    },
    {
      id: 'field',
      kind: 'assert',
      name: 'Pole inna_nazwa',
      position: { x: 680, y: 290 },
      inputs: { actual: reference('product', 'product', 'promotion.inna_nazwa') },
      config: { predicate: 'notEmpty' },
    },
  ];
  const cleanup = request(
    'delete-cart',
    'Usuń koszyk',
    '/carts/:id',
    { id: reference('cart', 'cart', 'id') },
    { deleted: true },
    'DELETE',
  );
  const verifyCleanup: TestNode = {
    id: 'verify-cleanup',
    kind: 'assert',
    name: 'Koszyk został usunięty',
    position: { x: 400, y: 80 },
    inputs: { actual: reference('delete-cart', 'status'), expected: literal(200) },
    config: { predicate: 'equals' },
  };
  return {
    version: 1,
    id: 'shop',
    name: 'Sklep demonstracyjny',
    contracts: [
      { id: 'Product', name: 'Produkt', sample: product },
      { id: 'Cart', name: 'Koszyk', sample: { id: 'cart-1', total: 80 } },
    ],
    components: [productComponent, activeComponent, basketComponent],
    environment: { baseUrl: 'http://127.0.0.1:4318/demo/shop', variables: {}, secretNames: [] },
    scenarios: [
      {
        id: 'promotion',
        name: 'Promocja i cena w koszyku',
        description:
          'Sprawdź promocję, cenę końcową i dodatkowe pole. Dane demonstracyjne można uruchomić na lokalnym API.',
        graph: { nodes, edges: [] },
        cleanup: { nodes: [cleanup, verifyCleanup], edges: [] },
        parameters: {},
        errorPolicy: 'continue',
      },
    ],
  };
}

export function createTicketsProject(): Project {
  const sample: Json = { id: 7, title: 'Problem z logowaniem', status: 'open', priority: 'high' };
  const component: ComponentDefinition = {
    id: 'get-ticket',
    revision: 1,
    name: 'Zgłoszenie',
    description: 'Odczytaj zgłoszenie po identyfikatorze.',
    inputs: [{ name: 'id', type: 'number', defaultValue: 7, exposed: true, required: true }],
    outputs: [
      { name: 'ticket', type: 'object', contractId: 'Ticket', binding: reference('fetch', 'body') },
    ],
    graph: {
      nodes: [request('fetch', 'Pobierz zgłoszenie', '/tickets/:id', { id: param('id') }, sample)],
      edges: [],
    },
  };
  const action: ComponentDefinition = {
    id: 'ticket-open',
    revision: 1,
    name: 'Sprawdź otwarte zgłoszenie',
    description: 'Sprawdź status zgłoszenia.',
    inputs: [{ name: 'ticket', type: 'object', contractId: 'Ticket', required: true }],
    outputs: [{ name: 'passed', type: 'boolean', binding: reference('check', 'passed') }],
    action: { input: 'ticket', type: 'object', contractId: 'Ticket' },
    graph: {
      nodes: [
        {
          id: 'status',
          kind: 'select',
          name: 'Odczytaj status',
          inputs: { value: param('ticket') },
          position: { x: 60, y: 60 },
          config: { path: 'status' },
        },
        {
          id: 'check',
          kind: 'assert',
          name: 'Status otwarty',
          inputs: { actual: reference('status', 'value'), expected: literal('open') },
          position: { x: 380, y: 60 },
          config: { predicate: 'equals' },
        },
      ],
      edges: [],
    },
  };
  return {
    version: 1,
    id: 'tickets',
    name: 'System zgłoszeń',
    environment: { baseUrl: 'http://127.0.0.1:4318/demo/support', variables: {}, secretNames: [] },
    contracts: [{ id: 'Ticket', name: 'Zgłoszenie', sample }],
    components: [component, action],
    scenarios: [
      {
        id: 'ticket-status',
        name: 'Status i priorytet zgłoszenia',
        description: 'Drugi projekt: te same operacje, inne endpointy i własne akcje.',
        parameters: {},
        errorPolicy: 'continue',
        cleanup: empty(),
        graph: {
          nodes: [
            {
              id: 'ticket',
              kind: 'component',
              name: 'Zgłoszenie',
              position: { x: 60, y: 160 },
              inputs: { id: literal(7) },
              config: { componentId: 'get-ticket', revision: 1 },
            },
            {
              id: 'open',
              kind: 'component',
              name: 'Sprawdź otwarte zgłoszenie',
              position: { x: 410, y: 60 },
              inputs: { ticket: reference('ticket', 'ticket') },
              config: { componentId: 'ticket-open', revision: 1 },
            },
            {
              id: 'priority',
              kind: 'assert',
              name: 'Priorytet wysoki',
              position: { x: 410, y: 310 },
              inputs: {
                actual: reference('ticket', 'ticket', 'priority'),
                expected: literal('high'),
              },
              config: { predicate: 'equals' },
            },
          ],
          edges: [],
        },
      },
    ],
  };
}
export function emptyScenario(name = 'Nowy scenariusz'): Scenario {
  return {
    id: uid('scenario'),
    name,
    description: '',
    graph: { nodes: [createNode('trigger', { x: 80, y: 60 })], edges: [] },
    cleanup: empty(),
    parameters: {},
    errorPolicy: 'continue',
  };
}
export function createEmptyProject(): Project {
  return {
    version: 1,
    id: uid('project'),
    name: 'Mój projekt API',
    scenarios: [emptyScenario()],
    components: [],
    contracts: [],
    environment: { baseUrl: '', variables: {}, secretNames: [] },
  };
}

/** A complete test assembled from value-producing blocks and one generic assertion per rule. */
export function createBlocksProject(): Project {
  const op = (operator: Operator, left: Binding, right?: Binding): Binding => ({
    kind: 'expression',
    operator,
    inputs: { left, ...(right ? { right } : {}) },
  });
  const source = request(
    'response',
    'Odczytaj dane API',
    '/products/:code',
    { code: literal(123) },
    product,
  );
  const field = (path: string) => reference(source.id, 'body', path);
  const named: TestNode = {
    id: 'name-check',
    kind: 'assert',
    name: 'Nazwa jest niepustym tekstem',
    position: { x: 470, y: 30 },
    config: { predicate: 'equals' },
    inputs: {
      actual: op(
        'and',
        op('equals', op('typeOf', field('name')), literal('string')),
        op('greater', op('length', field('name')), literal(0)),
      ),
      expected: literal(true),
    },
  };
  const discount: TestNode = {
    id: 'discount-check',
    kind: 'assert',
    name: 'Aktywna promocja daje minimum 10%',
    position: { x: 470, y: 350 },
    config: { predicate: 'equals' },
    inputs: {
      actual: op(
        'and',
        field('promotion.active'),
        op('atMost', field('promotion.price'), op('multiply', field('price'), literal(0.9))),
      ),
      expected: literal(true),
    },
  };
  source.position = { x: 60, y: 170 };
  return {
    version: 1,
    id: 'blocks-poc',
    name: 'Bloki operatorów · PoC',
    components: [],
    contracts: [],
    environment: { baseUrl: 'http://127.0.0.1:4318/demo/shop', variables: {}, secretNames: [] },
    scenarios: [
      {
        id: 'blocks-demo',
        name: 'Test z zagnieżdżonych bloków',
        description:
          'Odczyt API → typ i długość nazwy oraz reguła ceny → porównanie z true. Zielone bloki zwracają wartości; Porównanie zalicza test.',
        parameters: {},
        errorPolicy: 'continue',
        graph: { nodes: [source, named, discount], edges: [] },
        cleanup: { nodes: [], edges: [] },
      },
    ],
  };
}

/** Control wires enforce prices → create cart → add products; references carry current values. */
export function createWorkflowProject(): Project {
  const start = { ...createNode('trigger', { x: 40, y: 100 }), id: 'flow-start' };
  const prices = request(
    'flow-prices',
    'Pobierz ceny produktów',
    '/products/123',
    {},
    product,
    'GET',
    370,
    100,
  );
  const branch = {
    ...createNode('if', { x: 710, y: 100 }),
    id: 'flow-if',
    name: 'IF response = 200',
    inputs: { actual: reference(prices.id, 'status'), expected: literal(200) },
  };
  const cart = request(
    'flow-cart',
    'Utwórz pusty koszyk',
    '/carts',
    { body: literal({}) },
    { id: 'demo-cart', total: 0, quantity: 0, items: [] },
    'POST',
    1050,
    20,
  );
  cart.config.sampleStatus = 201;
  const add = request(
    'flow-add',
    'Dodaj produkty do koszyka',
    '/carts/{{cartId}}/items',
    {
      cartId: reference(cart.id, 'body', 'id'),
      body: literal({}),
    },
    {
      id: 'demo-cart',
      total: 160,
      quantity: 2,
      items: [{ productId: 123, quantity: 2, price: 80 }],
    },
    'POST',
    1390,
    20,
  );
  const body = {
    ...createNode('build', { x: 1050, y: 540 }),
    id: 'flow-body',
    name: 'Produkty do dodania',
    inputs: { productId: reference(prices.id, 'body', 'id'), quantity: literal(2) },
  };
  add.inputs.body = reference(body.id, 'value');
  const check = {
    ...createNode('assert', { x: 1730, y: 20 }),
    id: 'flow-check',
    name: 'Porównaj sumę z pobraną ceną',
    inputs: {
      actual: reference(add.id, 'body', 'total'),
      expected: {
        kind: 'expression' as const,
        operator: 'multiply' as const,
        inputs: { left: reference(prices.id, 'body', 'promotion.price'), right: literal(2) },
      },
    },
  };
  const otherwise = {
    ...createNode('assert', { x: 1050, y: 920 }),
    id: 'flow-else',
    name: 'Zgłoś brak cen — bez tworzenia koszyka',
    inputs: { actual: reference(prices.id, 'status'), expected: literal(200) },
  };
  const cleanup = request(
    'flow-cleanup',
    'Usuń utworzony koszyk',
    '/carts/{{cartId}}',
    { cartId: reference(cart.id, 'body', 'id') },
    { deleted: true },
    'DELETE',
  );
  const pairs = [
    [start.id, prices.id],
    [prices.id, branch.id],
    [cart.id, add.id],
    [add.id, check.id],
  ];
  return {
    version: 1,
    id: 'workflow-poc',
    name: 'Przepływ: ceny i koszyk',
    components: [],
    contracts: [],
    environment: { baseUrl: 'http://127.0.0.1:4318/demo/shop', variables: {}, secretNames: [] },
    scenarios: [
      {
        id: 'workflow-cart',
        name: 'Ceny → koszyk → produkty',
        description:
          'Ręczny Start, kolejność requestów i IF / ELSE. Zmień endpoint produktu na /products/999, aby sprawdzić gałąź Nie.',
        parameters: {},
        errorPolicy: 'continue',
        graph: {
          nodes: [start, prices, branch, cart, body, add, check, otherwise],
          edges: [
            ...pairs.map(([source, target]) => ({ id: uid('order'), source, target })),
            { id: 'flow-yes', source: branch.id, target: cart.id, branch: 'true' },
            { id: 'flow-no', source: branch.id, target: otherwise.id, branch: 'false' },
          ],
        },
        cleanup: { nodes: [cleanup], edges: [] },
      },
    ],
  };
}

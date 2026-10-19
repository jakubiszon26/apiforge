import { Router } from 'express';
import { randomUUID } from 'node:crypto';

// Isolated example API, not used by the execution engine.
export const demoRouter = Router();
const carts = new Map<
  string,
  {
    id: string;
    productId?: number;
    quantity: number;
    items?: { productId: number; quantity: number; price: number }[];
    total: number;
    createdAt: number;
  }
>();
demoRouter.get('/shop/products/:id', (req, res) => {
  const id = Number(req.params.id);
  if (![123, 124].includes(id)) {
    res.status(404).json({ error: 'Nie znaleziono produktu' });
    return;
  }
  res.json({
    id,
    name: 'Kawa ziarnista',
    price: 100,
    promotion: {
      type: 'fixed_price',
      active: true,
      price: id === 123 ? 80 : 90,
      inna_nazwa: 'Oferta jesienna',
    },
  });
});
demoRouter.post('/shop/carts', (req, res) => {
  for (const [id, cart] of carts) if (Date.now() - cart.createdAt > 3600000) carts.delete(id);
  const { productId, quantity } = req.body ?? {};
  const emptyCart = productId === undefined && quantity === undefined;
  if (
    !emptyCart &&
    (![123, 124].includes(Number(productId)) ||
      !Number.isInteger(quantity) ||
      quantity < 1 ||
      quantity > 100)
  ) {
    res.status(400).json({ error: 'Nieprawidłowy produkt lub ilość' });
    return;
  }
  if (carts.size >= 1000) {
    res.status(429).json({ error: 'Za dużo koszyków demonstracyjnych' });
    return;
  }
  const cart = {
    id: randomUUID(),
    ...(emptyCart ? { items: [] } : { productId: Number(productId) }),
    quantity: emptyCart ? 0 : quantity,
    total: emptyCart ? 0 : (Number(productId) === 123 ? 80 : 90) * quantity,
    createdAt: Date.now(),
  };
  carts.set(cart.id, cart);
  res.status(201).json(cart);
});
// Separate creation and adding items makes the ordered workflow observable.
demoRouter.post('/shop/carts/:id/items', (req, res) => {
  const cart = carts.get(String(req.params.id));
  if (!cart) {
    res.status(404).json({ error: 'Najpierw utwórz koszyk' });
    return;
  }
  const { productId, quantity } = req.body ?? {};
  if (
    ![123, 124].includes(Number(productId)) ||
    !Number.isInteger(quantity) ||
    quantity < 1 ||
    quantity > 100
  ) {
    res.status(400).json({ error: 'Nieprawidłowy produkt lub ilość' });
    return;
  }
  if (!cart.items || cart.items.length >= 100) {
    res.status(400).json({ error: 'Koszyk nie obsługuje kolejnych pozycji' });
    return;
  }
  const price = Number(productId) === 123 ? 80 : 90;
  cart.items.push({ productId: Number(productId), quantity, price });
  cart.quantity += quantity;
  cart.total += price * quantity;
  res.json(cart);
});
demoRouter.get('/shop/carts/:id', (req, res) => {
  const cart = carts.get(String(req.params.id));
  if (cart) res.json(cart);
  else res.status(404).json({ error: 'Brak koszyka' });
});
demoRouter.delete('/shop/carts/:id', (req, res) => {
  const deleted = carts.delete(String(req.params.id));
  res.status(deleted ? 200 : 404).json({ deleted });
});
demoRouter.get('/support/tickets/:id', (req, res) => {
  const id = Number(req.params.id);
  if (id !== 7) {
    res.status(404).json({ error: 'Brak zgłoszenia' });
    return;
  }
  res.json({ id, title: 'Problem z logowaniem', status: 'open', priority: 'high' });
});

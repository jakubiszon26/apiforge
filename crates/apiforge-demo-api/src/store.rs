//! In-memory demo data: the product catalogue, the support ticket and bounded carts.

use serde::Serialize;
use serde_json::{Map, Value, json};
use std::{
    collections::HashMap,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

/// Maximum number of carts kept at once.
pub const MAX_CARTS: usize = 1000;
/// Carts older than this are discarded.
pub const CART_TTL: Duration = Duration::from_secs(60 * 60);
/// Maximum number of item lines in one cart.
pub const MAX_ITEMS: usize = 100;
/// Largest quantity accepted for one line.
const MAX_QUANTITY: i64 = 100;

/// Promotional unit price of a known product.
pub fn promotional_price(product_id: u32) -> Option<i64> {
    match product_id {
        123 => Some(80),
        124 => Some(90),
        _ => None,
    }
}

/// The catalogue entry for a known product.
pub fn product(product_id: u32) -> Option<Value> {
    let price = promotional_price(product_id)?;
    Some(json!({
        "id": product_id,
        "name": "Whole bean coffee",
        "price": 100,
        "promotion": {
            "type": "fixed_price",
            "active": true,
            "price": price,
            "inna_nazwa": "Autumn offer",
        },
    }))
}

/// The only ticket in the demo support system.
pub fn ticket(ticket_id: f64) -> Option<Value> {
    (ticket_id == 7.0).then(
        || json!({ "id": 7, "title": "Cannot sign in", "status": "open", "priority": "high" }),
    )
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Item {
    pub product_id: u32,
    pub quantity: i64,
    pub price: i64,
}

/// A cart either holds one product line from creation, or starts empty and accepts
/// item lines afterwards.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Cart {
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub items: Option<Vec<Item>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub product_id: Option<u32>,
    pub quantity: i64,
    pub total: i64,
    /// Creation time in milliseconds since the Unix epoch.
    pub created_at: u64,
    #[serde(skip)]
    created: Instant,
}

/// Why a cart operation was refused.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CartError {
    InvalidLine,
    TooManyCarts,
    NotFound,
    ItemsNotSupported,
}

/// Validated product line: product and quantity.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Line {
    pub product_id: u32,
    pub quantity: i64,
}

impl Line {
    /// Accepts a known product (numeric or numeric text, as the reference coerces it)
    /// and an integer quantity from 1 to 100.
    pub fn parse(fields: &Map<String, Value>) -> Option<Self> {
        let product_id = fields.get("productId").map_or(f64::NAN, js_number);
        let product_id = [123, 124]
            .into_iter()
            .find(|&known| f64::from(known) == product_id)?;
        let quantity = fields.get("quantity").and_then(Value::as_f64)?;
        let integral = quantity.fract() == 0.0 && (1.0..=MAX_QUANTITY as f64).contains(&quantity);
        integral.then_some(Self {
            product_id,
            quantity: quantity as i64,
        })
    }
}

/// `Number(value)` of the reference implementation for JSON scalars; other values
/// are not numbers.
pub fn js_number(value: &Value) -> f64 {
    match value {
        Value::Number(number) => number.as_f64().unwrap_or(f64::NAN),
        Value::String(text) => js_number_from_str(text),
        Value::Bool(flag) => f64::from(u8::from(*flag)),
        Value::Null => 0.0,
        Value::Array(_) | Value::Object(_) => f64::NAN,
    }
}

/// `Number(text)` for decimal text; whitespace-only text is zero.
pub fn js_number_from_str(text: &str) -> f64 {
    let text = text.trim();
    if text.is_empty() {
        return 0.0;
    }
    let finite = text
        .bytes()
        .all(|byte| byte.is_ascii_digit() || b"+-.eE".contains(&byte));
    if finite {
        text.parse().unwrap_or(f64::NAN)
    } else {
        f64::NAN
    }
}

#[derive(Debug, Default)]
pub struct Store {
    carts: HashMap<String, Cart>,
}

impl Store {
    /// Creates an empty cart (`line` is `None`) or a cart holding one product line.
    pub fn create(&mut self, line: Option<Line>, now: Instant) -> Result<Cart, CartError> {
        self.purge(now);
        if self.carts.len() >= MAX_CARTS {
            return Err(CartError::TooManyCarts);
        }
        let (items, product_id, quantity, total) = match line {
            None => (Some(vec![]), None, 0, 0),
            Some(line) => (
                None,
                Some(line.product_id),
                line.quantity,
                line_price(line) * line.quantity,
            ),
        };
        let cart = Cart {
            id: uuid::Uuid::new_v4().to_string(),
            items,
            product_id,
            quantity,
            total,
            created_at: unix_millis(),
            created: now,
        };
        self.carts.insert(cart.id.clone(), cart.clone());
        Ok(cart)
    }

    /// Appends a line to a cart created empty.
    pub fn add_item(&mut self, id: &str, line: Line, now: Instant) -> Result<Cart, CartError> {
        let cart = self.live(id, now).ok_or(CartError::NotFound)?;
        let items = match &mut cart.items {
            Some(items) if items.len() < MAX_ITEMS => items,
            _ => return Err(CartError::ItemsNotSupported),
        };
        let price = line_price(line);
        items.push(Item {
            product_id: line.product_id,
            quantity: line.quantity,
            price,
        });
        cart.quantity += line.quantity;
        cart.total += price * line.quantity;
        Ok(cart.clone())
    }

    pub fn get(&mut self, id: &str, now: Instant) -> Option<Cart> {
        self.live(id, now).cloned()
    }

    /// Whether a live cart was deleted.
    pub fn delete(&mut self, id: &str, now: Instant) -> bool {
        self.live(id, now).is_some() && self.carts.remove(id).is_some()
    }

    #[cfg(test)]
    pub fn len(&self) -> usize {
        self.carts.len()
    }

    #[cfg(test)]
    pub fn is_empty(&self) -> bool {
        self.carts.is_empty()
    }

    /// A cart that has not expired; an expired one is removed on access.
    fn live(&mut self, id: &str, now: Instant) -> Option<&mut Cart> {
        if self.carts.get(id).is_some_and(|cart| expired(cart, now)) {
            self.carts.remove(id);
        }
        self.carts.get_mut(id)
    }

    fn purge(&mut self, now: Instant) {
        self.carts.retain(|_, cart| !expired(cart, now));
    }
}

fn line_price(line: Line) -> i64 {
    promotional_price(line.product_id).unwrap_or_default()
}

fn expired(cart: &Cart, now: Instant) -> bool {
    now.saturating_duration_since(cart.created) > CART_TTL
}

fn unix_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_millis() as u64)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(product_id: u32, quantity: i64) -> Option<Line> {
        Some(Line {
            product_id,
            quantity,
        })
    }

    fn fields(value: Value) -> Map<String, Value> {
        value.as_object().cloned().expect("object")
    }

    #[test]
    fn lines_follow_the_reference_coercion() {
        let parse = |value| Line::parse(&fields(value));
        assert_eq!(
            parse(json!({ "productId": 123, "quantity": 2 })),
            line(123, 2)
        );
        assert_eq!(
            parse(json!({ "productId": "124", "quantity": 2.0 })),
            line(124, 2)
        );
        assert_eq!(
            parse(json!({ "productId": " 123 ", "quantity": 100 })),
            line(123, 100)
        );
        for invalid in [
            json!({ "productId": 125, "quantity": 1 }),
            json!({ "productId": 123, "quantity": 0 }),
            json!({ "productId": 123, "quantity": 101 }),
            json!({ "productId": 123, "quantity": 1.5 }),
            json!({ "productId": 123, "quantity": "2" }),
            json!({ "productId": null, "quantity": 1 }),
            json!({ "quantity": 1 }),
            json!({ "productId": 123 }),
        ] {
            assert_eq!(parse(invalid.clone()), None, "{invalid}");
        }
    }

    #[test]
    fn carts_expire_after_an_hour() {
        let mut store = Store::default();
        let start = Instant::now();
        let cart = store.create(line(123, 1), start).expect("cart");
        let later = start + CART_TTL;
        assert!(store.get(&cart.id, later).is_some());
        let expired = later + Duration::from_millis(1);
        assert!(store.get(&cart.id, expired).is_none());
        assert!(store.is_empty());
    }

    #[test]
    fn the_number_of_carts_is_bounded() {
        let mut store = Store::default();
        let now = Instant::now();
        for _ in 0..MAX_CARTS {
            store.create(None, now).expect("cart");
        }
        assert_eq!(
            store.create(None, now).unwrap_err(),
            CartError::TooManyCarts
        );
        // Expired carts make room again.
        let later = now + CART_TTL + Duration::from_secs(1);
        assert!(store.create(None, later).is_ok());
        assert_eq!(store.len(), 1);
    }

    #[test]
    fn item_lines_are_bounded_and_need_an_empty_cart() {
        let mut store = Store::default();
        let now = Instant::now();
        let single = store.create(line(123, 1), now).expect("cart");
        let item = Line {
            product_id: 124,
            quantity: 1,
        };
        assert_eq!(
            store.add_item(&single.id, item, now).unwrap_err(),
            CartError::ItemsNotSupported
        );
        let cart = store.create(None, now).expect("cart");
        for _ in 0..MAX_ITEMS {
            store.add_item(&cart.id, item, now).expect("item");
        }
        assert_eq!(
            store.add_item(&cart.id, item, now).unwrap_err(),
            CartError::ItemsNotSupported
        );
        let cart = store.get(&cart.id, now).expect("cart");
        assert_eq!((cart.quantity, cart.total), (100, 9000));
        assert_eq!(
            store.add_item("missing", item, now).unwrap_err(),
            CartError::NotFound
        );
    }
}

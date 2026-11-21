# DEMO-01: The first-start project shows the 0.2.0 language

- Size: M · Owner review: 30 min · Risk: `default` (what a new install
  contains), `visible UI`
- State: Analysis (issue #523) · proposal posted, **decision needed** (questions 9–11)
  · for 0.2.1 · found by the owner on the v0.2.0-rc.1 packages, 2026-11-21
- Analysis model: Fable 5.2 · Implementation model: —

## Goal

"The default project needs updating": its content is old. A tester who
starts the application for the first time sees one scenario that reads
top to bottom as the language is written since 0.2.0, runs green as a
simulation and against the demo API, and has every idea of the language
in it once.

## What is there today

A first start writes *Demo shop* from `demos::shop_project()`, a
translation of the reference editor's fixture
(`docs/qa/2026-11-21/DEMO-01/today.afs`, board `demo-today-1600.png`):

- the main flow is three component calls and no request: the first
  board a tester sees has no HTTP request on it;
- the text carries what the board printed, not what a person writes:
  `@config({ method: "DELETE", timeoutMs: 10000.0, … })`, `@label` on
  every line, `@required(true)`, `@exposed(true)`;
- `on_error continue` without a reason, `expected_price = 80 * 1`
  written before the step it belongs to, a field called `inna_nazwa`;
- no `param`, no `var`, no `if`, no type check, no comment, and the
  cleanup is the only place with a request.

## What the owner will see

- Default behaviour that changes (question 9): a first start opens
  *Demo shop* with the scenario **Buy coffee on promotion** — the text
  below — and one component, `product`, written by hand.
- New (question 10): a second scenario, **The cart refuses bad orders**,
  three refused orders under `on_error continue`.
- Removed (question 9): the components *Active promotion* and *Add to
  cart* leave the first-start project; the scenario *Promotion and cart
  price* is replaced.
- Migrated or converted (question 11): nothing. A data directory that
  already holds a project is not touched.

```afs
scenario "Buy coffee on promotion" {
  description "Looks a product up, puts two of it in a new cart and checks what the shop charges. Run it as a simulation, or live against the local demo API."
  on_error stop
  param productId = 123
  param quantity = 2
  begin = start
  // A component: the lookup every scenario of this shop needs.
  product = product@1(id: param.productId)
  // Values several steps read are saved once.
  var listPrice = product.price
  var promotion = product.promotion
  // A request written in full. The sample answers a simulation.
  cart = POST "/carts" {
    body { productId: param.productId, quantity: param.quantity }
    sample 201 { id: "sample-cart", productId: 123, quantity: 2, total: 160 }
  }
  // Checks stand under the step they check.
  created = assert cart.status == 201
  has_id = assert nonempty cart.body.id
  total_is_a_number = assert cart.body.total is "number"
  var cartId = cart.body.id
  var total = cart.body.total
  // One branch runs, the other is skipped.
  promoted = if promotion.active == true {
    discounted = assert total == promotion.price * param.quantity
    saving = listPrice * param.quantity - total
    saves = assert saving > 0
  } else {
    full_price = assert total == listPrice * param.quantity
  }
  cleanup {
    // The cleanup runs whatever happened above.
    removed = DELETE "/carts/{{cartId}}" {
      sample 200 { deleted: true }
    }
    gone = assert removed.body.deleted == true
  }
}
```

It shows, each once: `param`, `start` and the written order, a component
call, `var` and its chips, a request with a body and a sample, three
kinds of check under their step (one of them the type check of DIAG-08),
`if`/`else`, an expression, comments, `cleanup`. Measured on `main`
`dd4667f` with today's `product` component: `fmt --check` clean, `check`
without a warning, the simulation passes (19 lines, `full_price`
skipped), the live run against `apiforge-demo-api` passes. The second
scenario (`the-cart-refuses-bad-orders.afs`) passes `check` and the
simulation; the demo API answers its three orders with 400.

## Mock-up

Design review captures, `review_rel021_demo_proposed` on
`mock/rel-0.2.1` at `58710bc`, in `docs/qa/2026-11-21/DEMO-01/`: the
board tidied at 1600 × 1000 (`demo-proposed-1600.png`) and 960 × 540
(`demo-proposed-960.png`), after a simulation
(`demo-proposed-run-1600.png`), the Code view
(`demo-proposed-code-1600.png`); today's board (`demo-today-1600.png`).
The board's layout in the capture is *Tidy up*'s; the shipped demo keeps
a layout file made the same way.

## Acceptance criteria

Tests in `crates/apiforge-desktop/src/folder/acceptance_tests/demo01.rs`
and `crates/apiforge-cli/tests/demo01.rs`.

1. A first start in an empty data directory opens *Demo shop* with the
   approved scenario(s) and the component `product` at revision 1; the
   first scenario's tab is open. `demo01_criterion_1_first_start`.
2. Every file of the demo passes `fmt --check` and `check` with no
   diagnostic at all, warnings included. `demo01_criterion_2_clean`.
3. The simulation of each scenario passes; the live run against
   `apiforge_demo_api::spawn` passes and leaves no cart.
   `demo01_criterion_3_runs_both_ways`.
4. No demo file contains `@config`, `@inputs`, `@required`, `@exposed`
   or `@after`; saving from the board after moving a card adds none.
   `demo01_criterion_4_written_as_a_person_writes`.
5. On the first board no two cards overlap and the whole chain is in
   view at Fit, at 960 × 540 and 1600 × 1000.
   `demo01_criterion_5_first_board`.
6. A data directory that holds a project opens that project, unchanged
   byte for byte. `demo01_criterion_6_existing_project_untouched`.
7. `README.md`'s first-start paragraph and `docs/development.md` name
   the scenario. Design review of the first start at both sizes.

## Not in scope

`demos::shop_project()` as the fixture of some hundred tests, and
`fixtures/projects/*.json`: they stay; only what `activation::create`
writes changes. The legacy workspace window's demos. A tour or hints.

## Proposals beyond the request

| Proposal | Problem solved | Size, risk | Trade-offs | Where |
| --- | --- | --- | --- | --- |
| *New demo project* in the Project menu | A tester who edited the demo, or installed before 0.2.1, gets the fresh one without deleting the data directory | S · `visible UI` | One more menu entry; writes a second *Demo shop* folder | 0.2.1 if question 11 = B, else later |
| The demo as `.afs` files in `fixtures/demo/`, embedded at build and held to REL-05's condition 1 | The demo can no longer fall behind the language: every release checks it as it checks the reference's examples | inside this ticket | The first-start project is prepared from text, not from the graph builders | this ticket's implementation |
| The demo API's `inna_nazwa` becomes `label` | A leftover of the translation shows in every product response | S | Touches `apiforge-demo-api`, `fixtures/projects/shop.json` and the tests that read the field | later |

## Technical appendix

`folder/activation.rs` `create` (the only caller that decides the first
start); `apiforge_project::prepare_project`; the published revision of
`product`;
`apiforge-demo-api` routes `POST /carts`, `DELETE /carts/{id}`. Risk:
the first-start tests that name *promotion-and-cart-price.afs*
(`dog_tests.rs`, `tab_band_tests.rs`, LAY-33) either keep
`shop_project()` through the test harness, which most do, or follow the
new name; the implementation lists which.

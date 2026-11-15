# LAY-24: what the rails into the next step cost

Measured by `review_lay24_measures` on `mock/lay-24-after-mark` (`f778b9b`),
on the boards the desktop tidies at 1600 × 1000, compact cards: lines drawn ·
crossings (the router's bridges) · coincident pairs · ink (board px), as
`quality` and LAY-23's criterion 7 count them. *Rails* is every `@after` and
branch wire drawn; *into next* is how many of them enter the next step of
the spine. The three options leave out the same rails, so one column
stands for all of them.

| Board | Checks | Wires | Rails | Into next | Today | Without the rails into the next step |
| --- | --- | --- | --- | --- | --- | --- |
| `fan-out` | collapsed | Chips (the default) | 10 | 10 | 19 · 9 · 5 · 10 700 | 9 · 1 · 3 · 3 500 |
| `fan-out` | collapsed | All wires | 10 | 10 | 31 · 50 · 76 · 25 100 | 21 · 10 · 71 · 18 000 |
| `fan-out` | drawn | Chips | 10 | 10 | 32 · 16 · 23 · 17 400 | 22 · 1 · 15 · 9 400 |
| `fan-out` | drawn | All wires | 10 | 10 | 45 · 48 · 119 · 34 000 | 35 · 10 · 102 · 26 100 |
| `far-value` | collapsed | Chips (the default) | 11 | 11 | 16 · 16 · 2 · 9 600 | 5 · 1 · 0 · 2 200 |
| `far-value` | collapsed | All wires | 11 | 11 | 30 · 48 · 103 · 30 100 | 19 · 14 · 102 · 22 700 |
| `far-value` | drawn | Chips | 11 | 11 | 29 · 17 · 13 · 16 700 | 18 · 1 · 9 · 8 200 |
| `far-value` | drawn | All wires | 11 | 11 | 44 · 51 · 138 · 39 500 | 33 · 16 · 135 · 31 000 |
| `shop-30` | collapsed | Chips (the default) | 8 | 3 | 25 · 4 · 14 · 14 100 | 22 · 4 · 14 · 12 800 |
| `shop-30` | collapsed | All wires | 8 | 3 | 42 · 76 · 54 · 39 400 | 39 · 67 · 58 · 38 200 |
| `shop-30` | drawn | Chips | 9 | 3 | 36 · 13 · 23 · 21 000 | 33 · 11 · 23 · 19 600 |
| `shop-30` | drawn | All wires | 9 | 3 | 57 · 112 · 112 · 53 600 | 54 · 110 · 110 · 52 200 |

`far-value` is LAY-25's synthetic scenario (#335), not on `main` yet; the
mock branch carries a copy.

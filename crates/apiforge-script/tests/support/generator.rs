//! Random scenario sources: DAGs of every statement form over the shared
//! workspace fixture. Shared by the topology test and the semantic fuzzer.
use std::collections::BTreeSet;

pub fn random(state: &mut u64) -> usize {
    *state = state
        .wrapping_mul(6_364_136_223_846_793_005)
        .wrapping_add(1_442_695_040_888_963_407);
    (*state >> 33) as usize
}

#[derive(Clone, Copy, PartialEq)]
enum Kind {
    Start,
    Value,
    Request,
    Check,
    If,
    Instance,
}

/// Handles end in `z` so that no handle is a prefix of another; renaming by
/// plain text replacement is then exact.
struct Generator {
    state: u64,
    /// Handles visible to data references, per scope (main, cleanup).
    main: Vec<(String, Kind)>,
    cleanup: Vec<(String, Kind)>,
    next: usize,
}

impl Generator {
    fn pick(&mut self, n: usize) -> usize {
        random(&mut self.state) % n
    }
    fn chance(&mut self, percent: usize) -> bool {
        self.pick(100) < percent
    }
    fn handle(&mut self) -> String {
        self.next += 1;
        format!("n{}z", self.next)
    }
    fn visible(&self, cleanup: bool) -> Vec<(String, Kind)> {
        let mut all = self.main.clone();
        if cleanup {
            all.extend(self.cleanup.iter().cloned());
        }
        all.retain(|(_, kind)| *kind != Kind::Start);
        all
    }
    fn reference(&mut self, cleanup: bool) -> Option<String> {
        let visible = self.visible(cleanup);
        if visible.is_empty() {
            return None;
        }
        let (name, kind) = visible[self.pick(visible.len())].clone();
        Some(match kind {
            Kind::Value | Kind::Instance => match self.pick(3) {
                0 => name,
                1 => format!("{name}.id"),
                _ => format!("{name}[\"key with space\"][0]"),
            },
            Kind::Request => match self.pick(4) {
                0 => format!("{name}.status"),
                1 => format!("{name}.body.items[0].id"),
                2 => format!("{name}.headers[\"content-type\"]"),
                _ => format!("{name}.duration"),
            },
            Kind::Check | Kind::If => {
                format!("{name}.{}", ["passed", "value"][self.pick(2)])
            }
            Kind::Start => unreachable!(),
        })
    }
    fn leaf(&mut self, cleanup: bool) -> String {
        match self.pick(8) {
            0 => "null".into(),
            1 => "true".into(),
            2 => format!("{}", self.pick(1000)),
            3 => "-2.5".into(),
            4 => "\"żółć\"".into(),
            5 => "param.limitz".into(),
            _ => self.reference(cleanup).unwrap_or_else(|| "1".into()),
        }
    }
    fn expr(&mut self, depth: usize, cleanup: bool) -> String {
        if depth == 0 {
            return self.leaf(cleanup);
        }
        let d = depth - 1;
        match self.pick(11) {
            0 => format!("{} + {}", self.expr(d, cleanup), self.expr(d, cleanup)),
            1 => format!(
                "({} - {}) * 2",
                self.expr(d, cleanup),
                self.expr(d, cleanup)
            ),
            2 => format!("{} >= {}", self.expr(d, cleanup), self.expr(d, cleanup)),
            3 => format!("!({} && true)", self.expr(d, cleanup)),
            4 => format!(
                "{{ a: {}, \"b c\": [{}] }}",
                self.expr(d, cleanup),
                self.expr(d, cleanup)
            ),
            5 => format!("[{}, {}]", self.expr(d, cleanup), self.leaf(cleanup)),
            6 => format!("\"pre-{{{{{}}}}}-post\"", self.expr(d, cleanup)),
            7 => format!("round({}, 2)", self.expr(d, cleanup)),
            8 => format!("concat({}, \"x\")", self.expr(d, cleanup)),
            9 => format!("op.add(left: {})", self.expr(d, cleanup)),
            _ => self.leaf(cleanup),
        }
    }
    fn check(&mut self, cleanup: bool) -> String {
        let a = self.expr(1, cleanup);
        match self.pick(7) {
            0 => format!("{a} == {}", self.leaf(cleanup)),
            1 => format!("{a} != 3"),
            2 => format!("{a} ~= 1 within 0.5"),
            3 => format!("{a} contains \"x\""),
            4 => format!("exists {a}"),
            5 => format!("nonempty {a}"),
            _ => format!("{a} > {}", self.leaf(cleanup)),
        }
    }
    /// Ordering towards earlier statements of the same graph only: acyclic.
    fn after(&mut self, cleanup: bool, available: &[(String, Kind)]) -> String {
        if available.is_empty() || !self.chance(35) {
            return String::new();
        }
        let mut orders = BTreeSet::new();
        for _ in 0..=self.pick(2) {
            let (name, kind) = available[self.pick(available.len())].clone();
            orders.insert(if kind == Kind::If {
                format!("{name}.{}", ["then", "else"][self.pick(2)])
            } else {
                name
            });
        }
        let _ = cleanup;
        format!(
            "@after({})\n",
            orders.into_iter().collect::<Vec<_>>().join(", ")
        )
    }
    fn request(&mut self, cleanup: bool) -> String {
        let method = ["GET", "POST", "PUT", "DELETE"][self.pick(4)];
        let url = match self.reference(cleanup) {
            Some(r) if self.chance(60) => format!("\"/items/{{{{{r}}}}}\""),
            _ => "\"/items\"".into(),
        };
        let mut items = vec![];
        let argument = self.chance(50);
        if argument {
            let value = self.expr(1, cleanup);
            items.push(format!("argument idz = {value}"));
            if self.chance(50) {
                items.push("argument wrapz = { inner: input.idz }".into());
            }
            items.push("headers { \"X-Id\": \"id-{{input.idz}}\" }".into());
        }
        if self.chance(40) {
            let value = self.expr(1, cleanup);
            items.push(format!("query {{ limit: {value} }}"));
        }
        if method != "GET" && self.chance(60) {
            let value = if argument {
                "input.idz".to_owned()
            } else {
                self.expr(2, cleanup)
            };
            items.push(format!("body {{ value: {value} }}"));
        }
        if self.chance(30) {
            items.push("timeout 250ms".into());
        }
        if self.chance(50) {
            items.push("sample 200 { items: [{ id: 1 }] }".into());
        }
        if items.is_empty() {
            format!("{method} {url}")
        } else {
            format!("{method} {url} {{\n{}\n}}", items.join("\n"))
        }
    }
    /// Returns statements; registers handles only after their own form so that
    /// nothing references itself.
    fn statements(&mut self, count: usize, cleanup: bool, depth: usize) -> Vec<String> {
        let mut out = vec![];
        let mut graph_handles: Vec<(String, Kind)> = vec![];
        for _ in 0..count {
            let handle = self.handle();
            let after = self.after(cleanup, &graph_handles);
            let (text, kind) = match self.pick(if depth == 0 { 9 } else { 7 }) {
                0 if depth == 0 && !cleanup && self.main.iter().all(|(_, k)| *k != Kind::Start) => {
                    (format!("{handle} = start"), Kind::Start)
                }
                0..=2 => {
                    let var = if self.chance(25) { "var " } else { "" };
                    let value = self.expr(2, cleanup);
                    (format!("{after}{var}{handle} = {value}"), Kind::Value)
                }
                3 | 4 => {
                    let request = self.request(cleanup);
                    (format!("{after}{handle} = {request}"), Kind::Request)
                }
                5 => {
                    let check = self.check(cleanup);
                    (format!("{after}{handle} = assert {check}"), Kind::Check)
                }
                6 => {
                    let checks: Vec<_> = (0..=self.pick(2)).map(|_| self.check(cleanup)).collect();
                    let with = if self.chance(50) {
                        format!(" with {}", self.expr(1, cleanup))
                    } else {
                        String::new()
                    };
                    let mode = ["all", "any"][self.pick(2)];
                    (
                        format!(
                            "{after}{handle} = condition {mode} {{ {} }}{with}",
                            checks.join(", ")
                        ),
                        Kind::Check,
                    )
                }
                7 => {
                    let check = self.check(cleanup);
                    // The IF handle becomes visible before its branches, which may
                    // read its result; nested statements also order after it.
                    self.register(cleanup, &handle, Kind::If);
                    graph_handles.push((handle.clone(), Kind::If));
                    let n = 1 + self.pick(3);
                    let then = self.statements(n, cleanup, depth + 1);
                    let otherwise = if self.chance(50) {
                        let n = 1 + self.pick(2);
                        let branch = self.statements(n, cleanup, depth + 1);
                        format!(" else {{\n{}\n}}", branch.join("\n"))
                    } else {
                        String::new()
                    };
                    out.push(format!(
                        "{after}{handle} = if {check} {{\n{}\n}}{otherwise}",
                        then.join("\n")
                    ));
                    continue;
                }
                _ => {
                    let call = if self.chance(50) {
                        let id = self.expr(1, cleanup);
                        format!("produkt(id: {id})")
                    } else {
                        let product = self.expr(1, cleanup);
                        format!("aktywna_promocja@1(product: {product})")
                    };
                    (format!("{after}{handle} = {call}"), Kind::Instance)
                }
            };
            self.register(cleanup, &handle, kind);
            graph_handles.push((handle, kind));
            out.push(text);
        }
        out
    }
    fn register(&mut self, cleanup: bool, handle: &str, kind: Kind) {
        if cleanup {
            self.cleanup.push((handle.into(), kind));
        } else {
            self.main.push((handle.into(), kind));
        }
    }
}

pub fn generate(seed: u64) -> String {
    let mut generator = Generator {
        state: seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1,
        main: vec![],
        cleanup: vec![],
        next: 0,
    };
    let count = 1 + generator.pick(25);
    let main = generator.statements(count, false, 0);
    let cleanup = if generator.chance(40) {
        let n = 1 + generator.pick(4);
        let statements = generator.statements(n, true, 0);
        format!("cleanup {{\n{}\n}}\n", statements.join("\n"))
    } else {
        String::new()
    };
    let policy = ["stop", "continue"][generator.pick(2)];
    format!(
        "scenario \"Generated {seed}\" {{\non_error {policy}\nparam limitz = 10\n{}\n{cleanup}}}\n",
        main.join("\n")
    )
}

//! Masking of secrets in everything a run publishes.
//!
//! The engine keeps raw values for execution and only publishes redacted copies.
//! Secrets are learned as the run progresses: session secrets, declared secret
//! names, values under conventionally sensitive keys (including Bearer tokens), URL
//! passwords and sensitive query parameters. Each secret is masked in its raw,
//! `encodeURIComponent` and form-encoded forms; values under sensitive keys are
//! masked whole. Name heuristics cannot protect an arbitrary secret; they are a
//! safety net behind explicit secret declarations.

use apiforge_core::{
    format::{Binding, Graph, Node, NodeConfig, OptionalJson, Port},
    js::utf16_len,
    run::{RequestSnapshot, RunResult, RunSnapshot, StepResult},
    template::encode_uri_component,
};
use indexmap::{IndexMap, IndexSet};
use serde_json::Value;
use url::Url;

/// Replacement text of every masked value.
pub const REDACTED: &str = "[REDACTED]";

/// Whether a key conventionally holds a credential:
/// `authorization|cookie|password|secret|token|api[-_]?key`, case-insensitively.
pub fn is_sensitive_key(key: &str) -> bool {
    let key = key.to_ascii_lowercase();
    ["authorization", "cookie", "password", "secret", "token"]
        .iter()
        .any(|word| key.contains(word))
        || key.match_indices("api").any(|(index, _)| {
            let rest = &key[index + 3..];
            let rest = rest
                .strip_prefix('-')
                .or_else(|| rest.strip_prefix('_'))
                .unwrap_or(rest);
            rest.starts_with("key")
        })
}

/// The set of known secrets and the masking rules.
#[derive(Clone, Debug, Default)]
pub struct Redactor {
    secrets: IndexSet<String>,
    /// Encoded forms of every secret, longest secret first.
    forms: Vec<String>,
}

impl Redactor {
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers a secret; empty text is ignored because it would match everywhere.
    pub fn add(&mut self, secret: &str) {
        if secret.is_empty() || !self.secrets.insert(secret.to_owned()) {
            return;
        }
        let mut ordered: Vec<&String> = self.secrets.iter().collect();
        // Stable: secrets of equal length keep the order in which they were learned.
        ordered.sort_by_key(|secret| std::cmp::Reverse(utf16_len(secret)));
        self.forms = ordered
            .into_iter()
            .flat_map(|secret| {
                let form: String =
                    url::form_urlencoded::byte_serialize(secret.as_bytes()).collect();
                // A secret inserted raw into a query string is decoded as form data
                // and re-serialised when another parameter is set, so `a+b c` is
                // published as `a+b+c`. The reference implementation leaks this form.
                let reparsed: String =
                    url::form_urlencoded::byte_serialize(&decode_form(secret)).collect();
                [secret.clone(), encode_uri_component(secret), form, reparsed]
            })
            .collect();
        self.forms.dedup();
    }

    pub fn is_empty(&self) -> bool {
        self.secrets.is_empty()
    }

    /// Learns secrets from a JSON document: values under sensitive keys, Bearer
    /// tokens, and passwords and sensitive query values of `url`/`baseUrl` fields.
    pub fn collect(&mut self, value: &Value) {
        match value {
            Value::Array(items) => items.iter().for_each(|item| self.collect(item)),
            Value::Object(fields) => {
                for (key, value) in fields {
                    if let Value::String(text) = value {
                        if is_sensitive_key(key) && !text.is_empty() {
                            self.add(text);
                            if let (Some(scheme), Some(token)) = (text.get(..7), text.get(7..))
                                && scheme.eq_ignore_ascii_case("bearer ")
                            {
                                self.add(token);
                            }
                        }
                        if key.eq_ignore_ascii_case("url") || key.eq_ignore_ascii_case("baseUrl") {
                            self.collect_url_text(text);
                        }
                    }
                    self.collect(value);
                }
            }
            _ => {}
        }
    }

    /// Learns the password and sensitive query values of a possibly relative URL.
    fn collect_url_text(&mut self, text: &str) {
        let Ok(base) = Url::parse("http://placeholder.invalid") else {
            return;
        };
        // Invalid URLs are diagnosed by the request operation.
        if let Ok(url) = Url::options().base_url(Some(&base)).parse(text) {
            if let Some(password) = url.password().filter(|password| !password.is_empty()) {
                self.add(password);
                // A malformed escape ends the inspection, as in the reference.
                match decode_uri_component(password) {
                    Some(decoded) => self.add(&decoded),
                    None => return,
                }
            }
            self.collect_query(&url);
        }
    }

    /// Learns the values of sensitive query parameters.
    pub fn collect_query(&mut self, url: &Url) {
        for (name, value) in url.query_pairs() {
            if is_sensitive_key(&name) && !value.is_empty() {
                self.add(&value);
            }
        }
    }

    pub fn redact_str(&self, text: &str) -> String {
        let mut text = text.to_owned();
        for form in &self.forms {
            if text.contains(form.as_str()) {
                text = text.replace(form.as_str(), REDACTED);
            }
        }
        text
    }

    /// Masks secrets in strings and replaces values under sensitive keys entirely.
    pub fn redact_value(&self, value: &Value) -> Value {
        match value {
            Value::String(text) => Value::String(self.redact_str(text)),
            Value::Array(items) => {
                Value::Array(items.iter().map(|v| self.redact_value(v)).collect())
            }
            Value::Object(fields) => Value::Object(
                fields
                    .iter()
                    .map(|(key, value)| (key.clone(), self.redact_field(key, value)))
                    .collect(),
            ),
            other => other.clone(),
        }
    }

    fn redact_field(&self, key: &str, value: &Value) -> Value {
        if is_sensitive_key(key) {
            Value::String(REDACTED.to_owned())
        } else {
            self.redact_value(value)
        }
    }

    fn redact_values(&self, values: &IndexMap<String, Value>) -> IndexMap<String, Value> {
        values
            .iter()
            .map(|(key, value)| (key.clone(), self.redact_field(key, value)))
            .collect()
    }

    fn redact_texts(&self, values: &IndexMap<String, String>) -> IndexMap<String, String> {
        values
            .iter()
            .map(|(key, value)| {
                let value = if is_sensitive_key(key) {
                    REDACTED.to_owned()
                } else {
                    self.redact_str(value)
                };
                (key.clone(), value)
            })
            .collect()
    }

    fn redact_optional(&self, value: &OptionalJson) -> OptionalJson {
        match value {
            OptionalJson::Missing => OptionalJson::Missing,
            OptionalJson::Present(value) => OptionalJson::Present(self.redact_value(value)),
        }
    }

    /// A redacted copy of a report. Enumerations are not text and stay as they are.
    pub fn redact_run(&self, run: &RunResult) -> RunResult {
        let text = |value: &str| self.redact_str(value);
        RunResult {
            id: text(&run.id),
            scenario_id: text(&run.scenario_id),
            mode: run.mode,
            started_at: text(&run.started_at),
            finished_at: run.finished_at.as_deref().map(text),
            status: run.status,
            steps: run
                .steps
                .iter()
                .map(|step| self.redact_step(step))
                .collect(),
            diagnostic: run.diagnostic,
            snapshot: run.snapshot.as_ref().map(|snapshot| RunSnapshot {
                graph: self.redact_graph(&snapshot.graph),
                cleanup: self.redact_graph(&snapshot.cleanup),
            }),
            error: run.error.as_deref().map(text),
            notice: run.notice.as_deref().map(text),
        }
    }

    pub fn redact_step(&self, step: &StepResult) -> StepResult {
        StepResult {
            node_id: self.redact_str(&step.node_id),
            name: self.redact_str(&step.name),
            kind: step.kind,
            status: step.status,
            phase: step.phase,
            inputs: self.redact_values(&step.inputs),
            outputs: self.redact_values(&step.outputs),
            duration_ms: step.duration_ms,
            message: step.message.as_deref().map(|text| self.redact_str(text)),
            expected: self.redact_optional(&step.expected),
            actual: self.redact_optional(&step.actual),
            request: step.request.as_ref().map(|request| RequestSnapshot {
                method: self.redact_str(&request.method),
                url: self.redact_str(&request.url),
                headers: self.redact_texts(&request.headers),
                body: self.redact_optional(&request.body),
            }),
            children: step
                .children
                .as_ref()
                .map(|children| children.iter().map(|step| self.redact_step(step)).collect()),
        }
    }

    fn redact_graph(&self, graph: &Graph) -> Graph {
        Graph {
            nodes: graph
                .nodes
                .iter()
                .map(|node| self.redact_node(node))
                .collect(),
            edges: graph
                .edges
                .iter()
                .map(|edge| {
                    let mut edge = edge.clone();
                    edge.id = self.redact_str(&edge.id);
                    edge.source = self.redact_str(&edge.source);
                    edge.target = self.redact_str(&edge.target);
                    edge
                })
                .collect(),
        }
    }

    fn redact_node(&self, node: &Node) -> Node {
        Node {
            id: self.redact_str(&node.id),
            kind: node.kind,
            name: self.redact_str(&node.name),
            position: node.position.clone(),
            inputs: self.redact_bindings(&node.inputs),
            config: self.redact_config(&node.config),
        }
    }

    fn redact_bindings(&self, bindings: &IndexMap<String, Binding>) -> IndexMap<String, Binding> {
        bindings
            .iter()
            .map(|(key, binding)| {
                let binding = if is_sensitive_key(key) {
                    // A typed report cannot hold text in place of a binding.
                    Binding::literal(Value::String(REDACTED.to_owned()))
                } else {
                    self.redact_binding(binding)
                };
                (key.clone(), binding)
            })
            .collect()
    }

    fn redact_binding(&self, binding: &Binding) -> Binding {
        match binding {
            Binding::Expression { operator, inputs } => Binding::Expression {
                operator: *operator,
                inputs: self.redact_bindings(inputs),
            },
            Binding::Literal { value } => Binding::literal(self.redact_value(value)),
            Binding::Reference {
                node_id,
                port,
                path,
            } => Binding::reference(
                self.redact_str(node_id),
                self.redact_str(port),
                self.redact_str(path),
            ),
            Binding::Parameter { name } => Binding::parameter(self.redact_str(name)),
            Binding::Environment { name } => Binding::environment(self.redact_str(name)),
        }
    }

    fn redact_port(&self, port: &Port) -> Port {
        Port {
            name: self.redact_str(&port.name),
            label: port.label.as_deref().map(|text| self.redact_str(text)),
            contract_id: port
                .contract_id
                .as_deref()
                .map(|text| self.redact_str(text)),
            default_value: self.redact_optional(&port.default_value),
            ..port.clone()
        }
    }

    fn redact_config(&self, config: &NodeConfig) -> NodeConfig {
        let text = |value: &Option<String>| value.as_deref().map(|value| self.redact_str(value));
        let ports = |ports: &Option<Vec<Port>>| {
            ports
                .as_ref()
                .map(|ports| ports.iter().map(|port| self.redact_port(port)).collect())
        };
        NodeConfig {
            url: text(&config.url),
            headers: config.headers.as_ref().map(|map| self.redact_texts(map)),
            query: config.query.as_ref().map(|map| self.redact_texts(map)),
            sample_response: self.redact_optional(&config.sample_response),
            contract_id: text(&config.contract_id),
            path: text(&config.path),
            component_id: text(&config.component_id),
            input_ports: ports(&config.input_ports),
            template_inputs: ports(&config.template_inputs),
            predicates: config.predicates.as_ref().map(|predicates| {
                predicates
                    .iter()
                    .map(|predicate| {
                        let mut predicate = predicate.clone();
                        predicate.input = self.redact_str(&predicate.input);
                        predicate.expected = self.redact_str(&predicate.expected);
                        predicate.path = text(&predicate.path);
                        predicate
                    })
                    .collect()
            }),
            ..config.clone()
        }
    }
}

/// The byte of a `%XX` escape starting at `index`, if there is a valid one.
fn percent_escape(bytes: &[u8], index: usize) -> Option<u8> {
    let hex = bytes.get(index + 1..index + 3)?;
    if bytes.get(index) != Some(&b'%') || !hex.iter().all(u8::is_ascii_hexdigit) {
        return None;
    }
    u8::from_str_radix(std::str::from_utf8(hex).ok()?, 16).ok()
}

/// Bytes of an `application/x-www-form-urlencoded` value: `+` is a space and valid
/// percent escapes are decoded; anything else is kept as it is.
fn decode_form(text: &str) -> Vec<u8> {
    let bytes = text.as_bytes();
    let mut output = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if let Some(byte) = percent_escape(bytes, index) {
            output.push(byte);
            index += 3;
        } else {
            output.push(if bytes[index] == b'+' {
                b' '
            } else {
                bytes[index]
            });
            index += 1;
        }
    }
    output
}

/// JavaScript `decodeURIComponent`, or `None` where it would throw.
pub(crate) fn decode_uri_component(text: &str) -> Option<String> {
    let bytes = text.as_bytes();
    let mut output = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' {
            output.push(percent_escape(bytes, index)?);
            index += 3;
        } else {
            output.push(bytes[index]);
            index += 1;
        }
    }
    String::from_utf8(output).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn sensitive_keys_follow_the_reference_pattern() {
        for key in [
            "Authorization",
            "set-cookie",
            "X-Api-Key",
            "api_key",
            "APIKEY",
            "accessToken",
            "clientSecret",
            "Password",
            "rapid-api-key-id",
        ] {
            assert!(is_sensitive_key(key), "{key}");
        }
        for key in ["apiVersion", "api--key", "keys", "author", "url", "pass"] {
            assert!(!is_sensitive_key(key), "{key}");
        }
    }

    #[test]
    fn secrets_are_masked_in_raw_and_encoded_forms_longest_first() {
        let mut redactor = Redactor::new();
        redactor.add("");
        assert!(redactor.is_empty());
        redactor.add("my+session secret");
        redactor.add("my");
        assert_eq!(
            redactor
                .redact_str("a=my+session secret&b=my%2Bsession%20secret&c=my%2Bsession+secret"),
            "a=[REDACTED]&b=[REDACTED]&c=[REDACTED]"
        );
        assert_eq!(redactor.redact_str("my own"), "[REDACTED] own");
        // Inserted raw into a query string and re-serialised as a form.
        assert_eq!(
            redactor.redact_str("?key=my+session+secret"),
            "?key=[REDACTED]"
        );
    }

    #[test]
    fn collection_finds_credentials_by_key_url_and_bearer() {
        let mut redactor = Redactor::new();
        redactor.collect(&json!({
            "headers": {"Authorization": "Bearer abc.def", "X-Other": "visible"},
            "environment": {"baseUrl": "https://user:p%40ss@example.com/?token=q-secret&page=2"},
            "nested": [{"url": "/relative?api-key=rel-secret"}, {"url": "http://[invalid"}],
            "empty": {"token": ""}
        }));
        let text = "Bearer abc.def abc.def p%40ss p@ss q-secret rel-secret visible 2";
        assert_eq!(
            redactor.redact_str(text),
            "[REDACTED] [REDACTED] [REDACTED] [REDACTED] [REDACTED] [REDACTED] visible 2"
        );
    }

    #[test]
    fn values_under_sensitive_keys_are_replaced_whole() {
        let mut redactor = Redactor::new();
        redactor.add("s3cret");
        assert_eq!(
            redactor.redact_value(&json!({
                "token": {"nested": 1},
                "list": ["x s3cret y", 2, null],
                "key": "s3cret"
            })),
            json!({"token": "[REDACTED]", "list": ["x [REDACTED] y", 2, null], "key": "[REDACTED]"})
        );
    }

    #[test]
    fn uri_component_decoding_rejects_malformed_escapes() {
        assert_eq!(decode_uri_component("a%20b%C5%BC").as_deref(), Some("a bż"));
        assert_eq!(decode_uri_component("a+b").as_deref(), Some("a+b"));
        assert_eq!(decode_uri_component("%zz"), None);
        assert_eq!(decode_uri_component("%C5"), None);
        assert_eq!(decode_uri_component("%2"), None);
        assert_eq!(decode_uri_component("%+1"), None);
        assert_eq!(decode_form("a+b%20c%zz%+1"), b"a b c%zz% 1");
    }
}

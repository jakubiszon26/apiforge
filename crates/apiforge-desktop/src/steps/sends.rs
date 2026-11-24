//! SENDS: what a request sends that is special (REQ-01a, the rules of SENDS).
//! Texts are compared as they are written, never as a run resolves them.

use super::model::Texts;
use apiforge_core::format::{Binding, Node, TemplatePart};
use apiforge_core::model::HttpMethod;
use indexmap::IndexMap;
use serde_json::Value;

/// The least group that has a common request.
const PEERS: usize = 3;
/// The most lines a card lists.
const LINES: usize = 4;
/// A `{{…}}` in a blanked URL; no text of a document holds it.
const BLANK: char = '\u{0}';

/// Where a request sends a text. A header's name is the same in any case.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
enum Place {
    Path,
    Body(String),
    Query(String),
    Header(String),
}

impl Place {
    /// The order the places are listed in.
    fn rank(&self) -> usize {
        match self {
            Self::Path => 0,
            Self::Body(_) => 1,
            Self::Query(_) => 2,
            Self::Header(_) => 3,
        }
    }
}

/// One text a request sends: its place, the place's name as the request
/// writes it, the text compared and the text shown.
#[derive(Clone, Debug)]
struct Sent {
    place: Place,
    name: String,
    text: String,
}

impl Sent {
    /// The line of the place; a path without its host unless `host`.
    fn line(&self, host: bool) -> Line {
        Line {
            text: match self.place {
                Place::Path if host => format!("path {}", self.text),
                Place::Path => format!("path {}", crate::report::url_path(&self.text)),
                _ => format!("{}: {}", self.name, self.text),
            },
            quiet: false,
        }
    }

    /// The scheme and host of a path as it is written; empty without them.
    fn host(&self) -> &str {
        let path = crate::report::url_path(&self.text);
        self.text.strip_suffix(path).unwrap_or(&self.text)
    }
}

/// One line of the column.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Line {
    pub text: String,
    /// The line says that there is nothing to say.
    pub quiet: bool,
}

/// What the column says of one request.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Sends {
    /// The common request in full, for the caption's hover; `None` when the
    /// request has too few peers to have one.
    pub common: Option<String>,
    pub lines: Vec<Line>,
}

/// How many of `count` lines a card lists, and how many `+ N more` counts.
pub fn cut(count: usize) -> (usize, usize) {
    if count <= LINES {
        (count, 0)
    } else {
        (LINES - 1, count - (LINES - 1))
    }
}

/// What each of `requests`, the requests of one section in written order,
/// sends that is special.
pub fn sends(requests: &[&Node], texts: &Texts<'_>) -> Vec<Sends> {
    let sent: Vec<Vec<Sent>> = requests.iter().map(|node| places(node, texts)).collect();
    let mut out: Vec<Option<Sends>> = vec![None; requests.len()];
    for group in groups(requests) {
        if group.len() < PEERS {
            for &index in &group {
                out[index] = Some(Sends {
                    common: None,
                    lines: own(&sent[index]),
                });
            }
            continue;
        }
        let common = common(group.iter().map(|&index| sent[index].as_slice()));
        // Where the group's hosts differ, the path line says the host.
        let mut hosts = group
            .iter()
            .flat_map(|&index| &sent[index])
            .filter(|sent| sent.place == Place::Path)
            .map(Sent::host);
        let first = hosts.next();
        let host = hosts.any(|host| Some(host) != first);
        let peers: Vec<&Node> = group.iter().map(|&index| requests[index]).collect();
        let full = full(&peers, &common);
        for &index in &group {
            out[index] = Some(Sends {
                common: Some(full.clone()),
                lines: differing(&sent[index], &common, host),
            });
        }
    }
    out.into_iter()
        .map(|sends| sends.expect("every request is in a group"))
        .collect()
}

/// The places of fewer than three peers: the request's own, without the path,
/// which the card's first line says, and without `Content-Type`.
fn own(sent: &[Sent]) -> Vec<Line> {
    let lines: Vec<Line> = sent
        .iter()
        .filter(|sent| {
            sent.place != Place::Path && sent.place != Place::Header("content-type".into())
        })
        .map(|sent| sent.line(true))
        .collect();
    if lines.is_empty() {
        vec![quiet("no body")]
    } else {
        lines
    }
}

fn quiet(text: &str) -> Line {
    Line {
        text: text.to_owned(),
        quiet: true,
    }
}

/// The text more than half of the group send, by place: a tie has none.
fn common<'s>(group: impl ExactSizeIterator<Item = &'s [Sent]>) -> IndexMap<Place, Sent> {
    let size = group.len();
    let mut counts: IndexMap<Place, IndexMap<&str, (usize, &Sent)>> = IndexMap::new();
    for sent in group.flatten() {
        counts
            .entry(sent.place.clone())
            .or_default()
            .entry(&sent.text)
            .or_insert((0, sent))
            .0 += 1;
    }
    let mut common: IndexMap<Place, Sent> = counts
        .into_iter()
        .filter_map(|(place, texts)| {
            let (_, sent) = texts.into_values().find(|(count, _)| count * 2 > size)?;
            Some((place, sent.clone()))
        })
        .collect();
    common.sort_by(|a, _, b, _| a.rank().cmp(&b.rank()));
    common
}

/// What one request sends differently from `common`, and what it leaves out.
fn differing(sent: &[Sent], common: &IndexMap<Place, Sent>, host: bool) -> Vec<Line> {
    let mut lines = Vec::new();
    for rank in 0..=3 {
        lines.extend(
            sent.iter()
                .filter(|sent| sent.place.rank() == rank)
                .filter(|sent| {
                    common
                        .get(&sent.place)
                        .is_none_or(|common| common.text != sent.text)
                })
                .map(|sent| sent.line(host)),
        );
        lines.extend(
            common
                .values()
                .filter(|common| common.place.rank() == rank)
                .filter(|common| !sent.iter().any(|sent| sent.place == common.place))
                .map(|common| Line {
                    text: format!("no {}", common.name),
                    quiet: false,
                }),
        );
    }
    if lines.is_empty() {
        lines.push(quiet("nothing differs"));
    }
    lines
}

/// The common request in full: its method and URL, the group's pattern where
/// the URLs have no common text, then its places.
fn full(peers: &[&Node], common: &IndexMap<Place, Sent>) -> String {
    // The pattern the group is of has the most `{{…}}`: those that join it
    // have literal segments in their place.
    let pattern = || {
        peers
            .iter()
            .map(|node| blanked(node))
            .max_by_key(|url| url.matches(BLANK).count())
            .unwrap_or_default()
    };
    let path = match common.get(&Place::Path) {
        Some(path) => path.text.clone(),
        None => pattern().replace(BLANK, "{{…}}"),
    };
    let mut text = format!("The common request\n{} {path}", method(peers[0]));
    for sent in common.values().filter(|sent| sent.place != Place::Path) {
        text.push('\n');
        text.push_str(&sent.line(true).text);
    }
    text
}

pub(super) fn method(node: &Node) -> &'static str {
    crate::labels::method(node.config.method.unwrap_or(HttpMethod::Get))
}

/// What `node` sends, in the order of the column.
fn places(node: &Node, texts: &Texts<'_>) -> Vec<Sent> {
    let mut out = Vec::new();
    if let Some(url) = node.inputs.get("url") {
        out.push(Sent {
            place: Place::Path,
            name: "path".into(),
            text: texts.url(url),
        });
    }
    let mut leaf = |input: &str, path: &str, text: String| {
        out.push(match input {
            "body" => Sent {
                place: Place::Body(path.to_owned()),
                name: if path.is_empty() { "body" } else { path }.to_owned(),
                text,
            },
            _ => {
                let (what, whole) = if input == "query" {
                    ("query", "query")
                } else {
                    ("header", "headers")
                };
                Sent {
                    place: if input == "query" {
                        Place::Query(path.to_owned())
                    } else {
                        Place::Header(path.to_lowercase())
                    },
                    name: if path.is_empty() {
                        whole.to_owned()
                    } else {
                        format!("{what} {path}")
                    },
                    text,
                }
            }
        });
    };
    for input in ["body", "query", "headers"] {
        if let Some(binding) = node.inputs.get(input) {
            // A query parameter or a header is one place, whatever it holds.
            let deep = input == "body";
            leaves(binding, "", deep, texts, &mut |path, text| {
                leaf(input, path, text);
            });
        }
    }
    out
}

/// The leaves of `binding` by path: the fields of an object, and of the
/// objects in it when `deep`.
fn leaves(
    binding: &Binding,
    path: &str,
    deep: bool,
    texts: &Texts<'_>,
    leaf: &mut impl FnMut(&str, String),
) {
    let inside = path.is_empty() || deep;
    match binding {
        Binding::Object { fields } if inside && !fields.is_empty() => {
            for (key, field) in fields {
                leaves(field, &joined(path, key), deep, texts, leaf);
            }
        }
        Binding::Literal {
            value: Value::Object(fields),
        } if inside && !fields.is_empty() => {
            for (key, value) in fields {
                let field = Binding::literal(value.clone());
                leaves(&field, &joined(path, key), deep, texts, leaf);
            }
        }
        other => leaf(path, texts.written(other)),
    }
}

fn joined(path: &str, key: &str) -> String {
    if path.is_empty() {
        key.to_owned()
    } else {
        format!("{path}.{key}")
    }
}

/// The URL with every `{{…}}` blanked.
fn blanked(node: &Node) -> String {
    match node.inputs.get("url") {
        None => String::new(),
        Some(Binding::Literal {
            value: Value::String(text),
        }) => text.clone(),
        Some(Binding::Template { parts }) => parts
            .iter()
            .map(|part| match part {
                TemplatePart::Text { text } => text.clone(),
                TemplatePart::Value { .. } => BLANK.to_string(),
            })
            .collect(),
        Some(_) => BLANK.to_string(),
    }
}

/// Whether a request of the URL `url` joins the group of `group`: the same
/// segments, but for literal ones exactly where the group has a `{{…}}`.
fn joins(url: &str, group: &str) -> bool {
    if url == group || url.split('/').count() != group.split('/').count() {
        return false;
    }
    url.split('/').zip(group.split('/')).all(|(own, theirs)| {
        own == theirs
            || (theirs.len() == BLANK.len_utf8()
                && theirs.starts_with(BLANK)
                && !own.is_empty()
                && !own.contains(BLANK))
    })
}

/// The groups of peers, in written order, as indexes into `requests`.
fn groups(requests: &[&Node]) -> Vec<Vec<usize>> {
    let mut exact: IndexMap<(&'static str, String), Vec<usize>> = IndexMap::new();
    for (index, node) in requests.iter().enumerate() {
        exact
            .entry((method(node), blanked(node)))
            .or_default()
            .push(index);
    }
    let shapes: Vec<&(&str, String)> = exact.keys().collect();
    // The group each joins, the first in written order; joining is a strict
    // order, so following it ends.
    let parent: Vec<Option<usize>> = shapes
        .iter()
        .map(|(method, url)| {
            shapes
                .iter()
                .position(|(other, group)| method == other && joins(url, group))
        })
        .collect();
    let mut merged: IndexMap<usize, Vec<usize>> = IndexMap::new();
    for (shape, indexes) in exact.values().enumerate() {
        let mut root = shape;
        while let Some(next) = parent[root] {
            root = next;
        }
        merged.entry(root).or_default().extend(indexes);
    }
    merged
        .into_values()
        .map(|mut group| {
            group.sort_unstable();
            group
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::steps::model::{Model, testing::scenario};

    /// What each request of `source` sends, by handle.
    fn of(source: &str) -> IndexMap<String, Sends> {
        let (project, scenario) = scenario(source);
        Model::of(&scenario.graph, Some(&scenario.cleanup), &project)
            .rows()
            .filter_map(|row| Some((row.handle.clone(), row.sends()?.clone())))
            .collect()
    }

    fn lines(sends: &Sends) -> Vec<&str> {
        sends.lines.iter().map(|line| line.text.as_str()).collect()
    }

    fn posts(bodies: &[&str]) -> String {
        let mut source = String::from("scenario \"S\" {\n  begin = start\n");
        for (index, body) in bodies.iter().enumerate() {
            source.push_str(&format!(
                "  r{index} = POST \"/orders\" {{ body {{ {body} }} }}\n"
            ));
        }
        source.push('}');
        source
    }

    /// Rule 2: more than half is common, and what differs is listed.
    #[test]
    fn more_than_half_of_the_group_is_common() {
        let sends = of(&posts(&["a: 1, b: 2", "a: 1, b: 3", "a: 2, b: 2"]));
        assert_eq!(lines(&sends["r0"]), ["nothing differs"]);
        assert!(sends["r0"].lines[0].quiet);
        assert_eq!(lines(&sends["r1"]), ["b: 3"]);
        assert_eq!(lines(&sends["r2"]), ["a: 2"]);
        assert_eq!(
            sends["r0"].common.as_deref(),
            Some("The common request\nPOST /orders\na: 1\nb: 2")
        );
    }

    /// Rule 2: half of the group is a tie, which has no common value: every
    /// card lists its own.
    #[test]
    fn a_tie_has_no_common_value() {
        let sends = of(&posts(&["a: 1", "a: 1", "a: 2", "a: 2"]));
        for (handle, expected) in [
            ("r0", "a: 1"),
            ("r1", "a: 1"),
            ("r2", "a: 2"),
            ("r3", "a: 2"),
        ] {
            assert_eq!(lines(&sends[handle]), [expected]);
        }
        assert_eq!(
            sends["r0"].common.as_deref(),
            Some("The common request\nPOST /orders")
        );
    }

    /// Rules 2 and 3: a place more than half leave out is listed by the cards
    /// that send it; a common place left out is `no …`.
    #[test]
    fn a_place_left_out() {
        let sends = of(&posts(&[
            "a: 1",
            "a: 1",
            "a: 1, extra: true",
            "extra: true",
        ]));
        assert_eq!(lines(&sends["r0"]), ["nothing differs"]);
        assert_eq!(lines(&sends["r2"]), ["extra: true"]);
        assert_eq!(lines(&sends["r3"]), ["extra: true", "no a"]);
    }

    /// Rule 4: a group of two lists its own places, the body first, without
    /// the path and `Content-Type`.
    #[test]
    fn fewer_than_three_peers_list_their_own_places() {
        let sends = of("scenario \"S\" {
  begin = start
  one = POST \"/orders/{{param.id}}\" {
    headers { \"Content-Type\": \"application/json\", Authorization: \"Bearer x\" }
    query { page: 2 }
    body { name: \"n\", deep: { at: 1 } }
  }
  two = POST \"/orders/{{param.other}}\" { headers { \"content-type\": \"text/plain\" } }
  param id = \"1\"
  param other = \"2\"
}");
        assert_eq!(
            lines(&sends["one"]),
            [
                "name: \"n\"",
                "deep.at: 1",
                "query page: 2",
                "header Authorization: \"Bearer x\""
            ]
        );
        assert_eq!(sends["one"].common, None);
        assert_eq!(lines(&sends["two"]), ["no body"]);
        assert!(sends["two"].lines[0].quiet);
    }

    /// Rule 1: the method and the URL with `{{…}}` blanked; a literal
    /// segment joins the first group that has `{{…}}` there.
    #[test]
    fn peers_are_of_the_method_and_the_blanked_url() {
        let sends = of("scenario \"S\" {
  begin = start
  param id = \"1\"
  a = GET \"/orders/{{param.id}}/items\"
  b = GET \"/orders/{{param.id}}/items\"
  literal = GET \"/orders/missing/items\"
  other_method = DELETE \"/orders/{{param.id}}/items\"
  other_path = GET \"/orders/{{param.id}}/lines\"
}");
        assert_eq!(lines(&sends["a"]), ["nothing differs"]);
        assert_eq!(lines(&sends["literal"]), ["path /orders/missing/items"]);
        assert_eq!(sends["other_method"].common, None);
        assert_eq!(sends["other_path"].common, None);
    }

    /// Review of the pull request: where the paths differ, the common request
    /// shows the pattern, whichever request is written first.
    #[test]
    fn the_common_request_shows_the_pattern_of_differing_paths() {
        let sends = of("scenario \"S\" {
  begin = start
  param id = \"1\"
  param other = \"2\"
  literal = GET \"https://shop.test/orders/missing/items\"
  a = GET \"https://shop.test/orders/{{param.id}}/items\"
  b = GET \"https://shop.test/orders/{{param.other}}/items\"
}");
        for sends in sends.values() {
            assert_eq!(
                sends.common.as_deref(),
                Some("The common request\nGET https://shop.test/orders/{{…}}/items")
            );
        }
    }

    /// Real-window test of 2026-11-24, 2: hosts are compared, and the path
    /// line says the host where the group's hosts differ.
    #[test]
    fn hosts_tell_requests_apart() {
        let sends = of("scenario \"S\" {
  begin = start
  a = GET \"https://eu.test/orders\"
  b = GET \"https://eu.test/orders\"
  c = GET \"https://us.test/orders\"
}");
        assert!(sends.values().all(|sends| sends.common.is_none()));
        let sends = of("scenario \"S\" {
  begin = start
  param host = \"eu.test\"
  a = GET \"https://{{param.host}}/orders\"
  b = GET \"https://{{param.host}}/orders\"
  c = GET \"https://us.test/orders\"
}");
        assert_eq!(lines(&sends["a"]), ["nothing differs"]);
        assert_eq!(lines(&sends["c"]), ["path https://us.test/orders"]);
    }

    #[test]
    fn a_literal_segment_joins_only_exactly_at_a_blank() {
        assert!(joins("/a/x/b", "/a/\u{0}/b"));
        assert!(!joins("/a/x/y/b", "/a/\u{0}/b"));
        assert!(!joins("/a/x/c", "/a/\u{0}/b"));
        assert!(!joins("/a//b", "/a/\u{0}/b"));
        assert!(!joins("/a/\u{0}/b", "/a/x/b"));
        assert!(!joins("/a/x", "/a/n-\u{0}"));
    }

    /// Rule 1: a Cleanup request is no peer of Main's.
    #[test]
    fn a_section_has_its_own_peers() {
        let sends = of("scenario \"S\" {
  begin = start
  a = POST \"/orders\" { body { n: 1 } }
  b = POST \"/orders\" { body { n: 1 } }
  cleanup { c = POST \"/orders\" { body { n: 1 } } }
}");
        assert_eq!(sends.len(), 3);
        assert!(sends.values().all(|sends| sends.common.is_none()));
    }

    /// Rule 1: texts are compared as they are written.
    #[test]
    fn texts_are_compared_as_written() {
        let sends = of(&posts(&["a: 2", "a: 2", "a: 1 + 1"]));
        assert_eq!(lines(&sends["r2"]), ["a: 1 + 1"]);
    }

    /// A header is the same place in any case of its name.
    #[test]
    fn a_headers_name_is_the_same_in_any_case() {
        let source = posts(&["n: 1", "n: 1", "n: 1"])
            .replacen("body", "headers { \"X-Key\": \"k\" }\n body", 1)
            .replacen(
                "POST \"/orders\" { body",
                "POST \"/orders\" { headers { \"x-key\": \"k\" }\n body",
                2,
            );
        let sends = of(&source);
        assert_eq!(sends.len(), 3);
        for sends in sends.values() {
            assert_eq!(lines(sends), ["nothing differs"], "{source}");
        }
    }

    /// Rule 5: four lines are listed, five are three and `+ 2 more`.
    #[test]
    fn more_than_four_lines_are_three_and_a_count() {
        assert_eq!(cut(0), (0, 0));
        assert_eq!(cut(4), (4, 0));
        assert_eq!(cut(5), (3, 2));
        assert_eq!(cut(9), (3, 6));
    }
}

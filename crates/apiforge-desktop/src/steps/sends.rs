//! SENDS: what a request sends that is special (REQ-01a, the rules of SENDS).

use super::model::Texts;
use apiforge_core::format::Node;

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
    /// The common request in full, for the caption's hover.
    pub common: Option<String>,
    pub lines: Vec<Line>,
}

pub fn cut(_count: usize) -> (usize, usize) {
    (0, 0)
}

pub fn sends(_requests: &[&Node], _texts: &Texts<'_>) -> Vec<Sends> {
    Vec::new()
}

fn joins(_url: &str, _group: &str) -> bool {
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::steps::model::{Model, testing::scenario};
    use indexmap::IndexMap;

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

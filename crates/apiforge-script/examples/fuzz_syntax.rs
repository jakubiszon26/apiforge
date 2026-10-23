//! Dependency-free mutational smoke fuzzer; not coverage-guided fuzzing.
//! Run: cargo run -p apiforge-script --example fuzz_syntax --release -- 60 1592639710
use apiforge_script::{format_tree, parse};
use std::time::{Duration, Instant};

fn random(state: &mut u64) -> usize {
    *state ^= *state << 13;
    *state ^= *state >> 7;
    *state ^= *state << 17;
    *state as usize
}

fn verify(source: &str) -> bool {
    let tree = parse(source);
    let mut end = 0;
    for token in &tree.tokens {
        assert_eq!(token.range.start, end);
        assert!(source.is_char_boundary(token.range.start));
        assert!(source.is_char_boundary(token.range.end));
        end = token.range.end;
    }
    assert_eq!(end, source.len());
    for diagnostic in &tree.diagnostics {
        assert!(diagnostic.range.start <= diagnostic.range.end);
        assert!(source.is_char_boundary(diagnostic.range.start));
        assert!(source.is_char_boundary(diagnostic.range.end));
        if let Some(fix) = &diagnostic.fix {
            assert!(fix.range.start <= fix.range.end);
            assert!(source.is_char_boundary(fix.range.start));
            assert!(source.is_char_boundary(fix.range.end));
        }
    }
    if tree.has_errors() {
        return false;
    }
    let formatted = match format_tree(&tree) {
        Ok(formatted) => formatted,
        Err(errors) => {
            assert!(
                errors
                    .iter()
                    .any(|error| error.message == "Formatted source would exceed the 1 MiB limit")
            );
            return true;
        }
    };
    let reparsed = parse(&formatted);
    assert!(
        !reparsed.has_errors(),
        "{formatted:?}: {:?}",
        reparsed.diagnostics
    );
    assert_eq!(format_tree(&reparsed).unwrap(), formatted);
    true
}

fn main() {
    let args: Vec<_> = std::env::args().skip(1).collect();
    assert!(
        args.len() <= 2,
        "Usage: fuzz_syntax [seconds] [nonzero-seed]"
    );
    let seconds: u64 = args.first().map_or(60, |s| s.parse().expect("seconds"));
    let seed: u64 = args
        .get(1)
        .map_or(1_592_639_710, |s| s.parse().expect("seed"));
    assert!(seconds > 0 && seed != 0);
    let alphabet: Vec<_> = "abcXYZ0129_ż🦀\0\n\r\t\"\\{}[](),:.@=+-*/%!?<>|&~ "
        .chars()
        .collect();
    let mut corpus: Vec<String> = vec![
        include_str!("../../../fixtures/scripts/checkout.afs").into(),
        include_str!("../../../fixtures/scripts/lookup.afs").into(),
        "scenario \"Templates\" { x = \"ż {{concat(\"a\", {v: param.x})}}\" }".into(),
        "scenario \"Paths\" { a = ? b = op.add(left: 1) c = a[\"key\"][0] }".into(),
        "scenario \"Checks\" { c = condition all { exists a, a ~= 1 within 0.1, } with a }".into(),
        "scenario \"Multiline\" { x = \"\"\"\r\n  first\r\n  {{param.x}}\r\n  last\r\n  \"\"\" }"
            .into(),
    ];
    for source in &corpus {
        assert!(verify(source));
    }
    let mut state = seed;
    let start = Instant::now();
    let mut cases = 0;
    let mut valid = 0;
    while start.elapsed() < Duration::from_secs(seconds) {
        let mut chars: Vec<_> = corpus[random(&mut state) % corpus.len()].chars().collect();
        for _ in 0..1 + random(&mut state) % 6 {
            let at = random(&mut state) % (chars.len() + 1);
            let character = alphabet[random(&mut state) % alphabet.len()];
            match random(&mut state) % 4 {
                0 => chars.insert(at, character),
                1 if at < chars.len() => {
                    chars.remove(at);
                }
                2 if at < chars.len() => chars[at] = character,
                _ if chars.len() < 4096 => {
                    let end = (at + random(&mut state) % 20).min(chars.len());
                    let duplicate = chars[at..end].to_vec();
                    chars.splice(at..at, duplicate);
                }
                _ => {}
            }
        }
        let source: String = chars.into_iter().collect();
        valid += usize::from(verify(&source));
        cases += 1;
        if source.len() < 8192 && random(&mut state).is_multiple_of(8) {
            if corpus.len() < 512 {
                corpus.push(source);
            } else {
                let index = 6 + random(&mut state) % (corpus.len() - 6);
                corpus[index] = source;
            }
        }
    }
    println!(
        "seed={seed} cases={cases} valid={valid} elapsed={:.2}s",
        start.elapsed().as_secs_f64()
    );
}

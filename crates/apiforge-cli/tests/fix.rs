//! LANG-20 (#481): `apiforge fix --folder <directory> [--check]` converts a
//! project written for 0.1.0 to language 2 whole, in one commit, or writes
//! nothing and lists what to change by hand.
//!
//! The language-1 project is the room booking as it was before LANG-13
//! (`tests/fixtures/rooms-language-1`). Its blocked variant has one
//! statement moved above the step it follows and one request nothing orders.
use apiforge_core::{
    format::Project,
    run::{RunMode, RunResult, StepResult},
};
use apiforge_mcp::executor::{self, CancellationToken, DEADLINE};
use apiforge_runner::RunRequest;
use apiforge_script::{Document, LowerContext, lexer::TokenKind, lower, parse};
use apiforge_storage::project::{Holder, HolderRole, MANIFEST_PATH, ProjectManifest, ProjectStore};
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
};

const BOOKING_DAY: &str = "scenarios/booking-day.afs";
const DOUBLE_BOOKING: &str = "scenarios/double-booking.afs";
const NOTICE: &str = "scenarios/double-booking-notice.afs";
const SIGN_IN: &str = "components/sign-in.afs";
const REVISION: &str = ".apiforge/revisions/sign-in/1.afs";
const SCENARIOS: [&str; 3] = [BOOKING_DAY, DOUBLE_BOOKING, NOTICE];
const SOURCES: [&str; 4] = [SIGN_IN, DOUBLE_BOOKING, NOTICE, BOOKING_DAY];
const HELD: &str = "The folder project is open in another writer session.";
const NOTHING_WRITTEN: &str = "Nothing was written.";

fn fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/rooms-language-1")
}

fn copy(from: &Path, to: &Path) {
    fs::create_dir_all(to).unwrap();
    for entry in fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        let target = to.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy(&entry.path(), &target);
        } else {
            fs::copy(entry.path(), target).unwrap();
        }
    }
}

/// A fresh copy of the language-1 project.
fn project() -> (tempfile::TempDir, PathBuf) {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path().join("rooms");
    copy(&fixture(), &root);
    (directory, root)
}

/// Every file under `root`, local data included; the writer's lock file by
/// its length, since Windows cannot read a held lock file.
fn tree(root: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    fn walk(root: &Path, directory: &Path, files: &mut BTreeMap<PathBuf, Vec<u8>>) {
        for entry in fs::read_dir(directory).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                walk(root, &path, files);
            } else {
                let relative = path.strip_prefix(root).unwrap().to_owned();
                let bytes = if path.file_name() == Some("project.lock".as_ref()) {
                    fs::metadata(&path).unwrap().len().to_le_bytes().to_vec()
                } else {
                    fs::read(&path).unwrap()
                };
                files.insert(relative, bytes);
            }
        }
    }
    let mut files = BTreeMap::new();
    walk(root, root, &mut files);
    files
}

/// Every file under `root` but its local data, which a writer's lock
/// creates: what a person sees changed.
fn files(root: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    let mut files = tree(root);
    files.retain(|path, _| !path.starts_with(".apiforge/local"));
    files
}

fn apiforge(args: &[&str], root: &Path) -> Output {
    Command::new(env!("CARGO_BIN_EXE_apiforge"))
        .args(
            args.iter()
                .map(|arg| arg.replace("{root}", &root.display().to_string())),
        )
        .output()
        .unwrap()
}

fn fix(root: &Path) -> Output {
    apiforge(&["fix", "--folder", "{root}"], root)
}

fn fix_check(root: &Path) -> Output {
    apiforge(&["fix", "--folder", "{root}", "--check"], root)
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

fn both(output: &Output) -> String {
    format!("{}{}", stdout(output), stderr(output))
}

fn read(root: &Path, path: &str) -> String {
    fs::read_to_string(root.join(path)).unwrap()
}

fn write(root: &Path, path: &str, text: &str) {
    fs::write(root.join(path), text).unwrap();
}

fn language_of(root: &Path) -> u32 {
    ProjectManifest::from_bytes(&fs::read(root.join(MANIFEST_PATH)).unwrap())
        .unwrap()
        .language()
}

/// The one-based line of the first occurrence of `needle` in `path`.
fn line_of(root: &Path, path: &str, needle: &str) -> usize {
    let text = read(root, path);
    let at = text
        .find(needle)
        .unwrap_or_else(|| panic!("{needle} in {path}"));
    text[..at].matches('\n').count() + 1
}

/// The steps a simulation runs, by handle, components' steps inside theirs.
fn steps(report: &RunResult) -> Vec<String> {
    fn walk(steps: &[StepResult], prefix: &str, out: &mut Vec<String>) {
        for step in steps {
            out.push(format!("{prefix}{} {:?}", step.handle, step.status));
            if let Some(children) = &step.children {
                walk(children, &format!("{prefix}{}/", step.handle), out);
            }
        }
    }
    let mut out = vec![];
    walk(&report.steps, "", &mut out);
    out
}

/// The simulation of each scenario as 0.1.0 reads the project: every source
/// in its stored spelling, `@after` included.
fn order_before(root: &Path) -> BTreeMap<&'static str, Vec<String>> {
    let manifest =
        ProjectManifest::from_bytes(&fs::read(root.join(MANIFEST_PATH)).unwrap()).unwrap();
    let mut project: Project = apiforge_core::demos::shop_project();
    project.scenarios.clear();
    project.components.clear();
    project.environment = manifest.environment.clone();
    let revision = lower(
        &parse(&read(root, REVISION)),
        &LowerContext::folder(&project).stored(),
    );
    let Some(Document::Component(sign_in)) = revision.document else {
        panic!("{:?}", revision.diagnostics);
    };
    project.components.push(sign_in);
    SCENARIOS
        .iter()
        .map(|path| {
            let lowered = lower(
                &parse(&read(root, path)),
                &LowerContext::folder(&project).stored(),
            );
            assert!(!lowered.has_errors(), "{path}: {:#?}", lowered.diagnostics);
            let Some(Document::Scenario(scenario)) = lowered.document else {
                panic!("{path}");
            };
            let mut run = project.clone();
            run.scenarios.push(scenario.clone());
            let outcome = executor::execute(
                RunRequest::new(run, scenario.id, RunMode::Simulation),
                None,
                CancellationToken::new(),
                DEADLINE,
            )
            .unwrap();
            (*path, steps(&outcome.report))
        })
        .collect()
}

/// The simulation of each scenario by `apiforge run --folder`.
fn order_after(root: &Path) -> BTreeMap<&'static str, Vec<String>> {
    SCENARIOS
        .iter()
        .map(|path| {
            let output = apiforge(
                &["run", "--folder", "{root}", "--scenario", path, "--json"],
                root,
            );
            assert_eq!(output.status.code(), Some(0), "{path}: {}", both(&output));
            let report: RunResult = serde_json::from_slice(&output.stdout).unwrap();
            (*path, steps(&report))
        })
        .collect()
}

/// The comments of `text`, in order.
fn comments(text: &str) -> Vec<String> {
    let tree = parse(text);
    tree.tokens
        .iter()
        .filter(|token| token.kind == TokenKind::Comment)
        .map(|token| text[token.range.clone()].trim().to_owned())
        .collect()
}

/// The blocked variant: `notice` moved above `first`, which it follows,
/// and a request nothing orders written below `rooms`.
fn block(root: &Path) {
    let text = read(root, NOTICE);
    let start = text.find("  @after(first)\n  notice = POST").unwrap();
    let end = start + text[start..].find("  sent = assert").unwrap();
    let moved = &text[start..end];
    let mut blocked = text.replacen(moved, "", 1);
    let first = blocked.find("  first = POST").unwrap();
    blocked.insert_str(first, moved);
    write(root, NOTICE, &blocked);
    let text = read(root, DOUBLE_BOOKING);
    write(
        root,
        DOUBLE_BOOKING,
        &text.replacen(
            "  slot = {",
            "  ping = GET \"/health\" {\n    sample 200 {}\n  }\n  slot = {",
            1,
        ),
    );
}

// Criterion 1 ----------------------------------------------------------------

/// Every source convertible: one run writes them and the manifest; each
/// changed file is listed with what was removed; `check` passes, `run` runs
/// and every scenario runs its steps in the order 0.1.0 ran them.
#[test]
fn lang20_criterion_1_every_source_converts_in_one_run() {
    let (_directory, root) = project();
    let before = order_before(&root);
    let afters: Vec<(&str, usize, String)> = SOURCES
        .iter()
        .flat_map(|path| {
            let text = read(&root, path);
            text.lines()
                .enumerate()
                .filter(|(_, line)| line.trim_start().starts_with("@after("))
                .map(|(index, line)| (*path, index + 1, line.trim().to_owned()))
                .collect::<Vec<_>>()
        })
        .collect();
    assert!(afters.len() > 10, "the fixture is written for 0.1.0");
    let output = fix(&root);
    assert_eq!(output.status.code(), Some(0), "{}", both(&output));
    let listed = stdout(&output);
    for (path, line, after) in &afters {
        assert!(
            listed.contains(&format!("{path}:{line}: removed `{after}`")),
            "{path}:{line} {after}\n{listed}"
        );
    }
    assert!(
        !listed.contains(SIGN_IN),
        "an unchanged source is not listed: {listed}"
    );
    assert!(
        listed.contains("apiforge.project.json: raised to language 2"),
        "{listed}"
    );
    for path in SOURCES {
        assert!(!read(&root, path).contains("@after"), "{path}");
    }
    let manifest =
        ProjectManifest::from_bytes(&fs::read(root.join(MANIFEST_PATH)).unwrap()).unwrap();
    assert_eq!(
        (manifest.format_version, manifest.language_version),
        (2, Some(2))
    );
    let check = apiforge(&["check", "--folder", "{root}"], &root);
    assert_eq!(check.status.code(), Some(0), "{}", both(&check));
    assert_eq!(order_after(&root), before);
}

// Criterion 2 ----------------------------------------------------------------

/// One source blocked: no file changes, exit code 1, and the report names
/// file, line, statement, reason and what to do for every blocked
/// statement. After the person's edit the same command converts.
#[test]
fn lang20_criterion_2_a_blocked_source_writes_nothing_and_names_every_statement() {
    let (_directory, root) = project();
    block(&root);
    let before = files(&root);
    let output = fix(&root);
    assert_eq!(output.status.code(), Some(1), "{}", both(&output));
    assert_eq!(files(&root), before, "no file and no manifest changes");
    let report = stderr(&output);
    let notice = line_of(&root, NOTICE, "  notice = POST");
    assert!(
        report.contains(&format!(
            "{NOTICE}:{notice}:3: `notice = POST \"/notifications\" {{`: This statement is not written where it runs today: it runs after `first`. Move its lines there."
        )),
        "{report}"
    );
    let ping = line_of(&root, DOUBLE_BOOKING, "  ping = GET");
    assert!(
        report.contains(&format!(
            "{DOUBLE_BOOKING}:{ping}:3: `ping = GET \"/health\" {{`: This step runs nowhere today: nothing orders it after `start`. Delete it, or write `@after(rooms)` on it; apiforge fix then removes that line."
        )),
        "{report}"
    );
    assert!(report.contains(NOTHING_WRITTEN), "{report}");
    assert!(report.contains("restore a source from Git"), "{report}");
    assert_eq!(language_of(&root), 1);

    // The person moves `notice` back and orders `ping` after `rooms`.
    write(
        &root,
        NOTICE,
        &fs::read_to_string(fixture().join(NOTICE)).unwrap(),
    );
    let text = read(&root, DOUBLE_BOOKING);
    write(
        &root,
        DOUBLE_BOOKING,
        &text.replacen("  ping = GET", "  @after(rooms)\n  ping = GET", 1),
    );
    let output = fix(&root);
    assert_eq!(output.status.code(), Some(0), "{}", both(&output));
    assert_eq!(language_of(&root), 2);
    let converted = read(&root, DOUBLE_BOOKING);
    assert!(converted.contains("  ping = GET"), "{converted}");
    assert!(!converted.contains("@after"), "{converted}");
}

// Criterion 3 ----------------------------------------------------------------

const LOOSE: &str = "scenario \"Double booking is refused\" {\n  begin = start\n  ping = GET \"/health\" {\n    sample 200 {}\n  }\n}\n";

/// A scenario with a `start` and a request nothing orders, and no `@after`
/// at all, blocks the project and names the request; with
/// `@after(<step above>)` on it the project converts and it runs there.
#[test]
fn lang20_criterion_3_a_scenario_without_after_is_judged() {
    let (_directory, root) = project();
    write(&root, DOUBLE_BOOKING, LOOSE);
    let output = fix(&root);
    assert_eq!(output.status.code(), Some(1), "{}", both(&output));
    let report = stderr(&output);
    assert!(
        report.contains(&format!(
            "{DOUBLE_BOOKING}:3:3: `ping = GET \"/health\" {{`: This step runs nowhere today"
        )),
        "{report}"
    );
    assert!(report.contains("write `@after(begin)` on it"), "{report}");
    assert_eq!(read(&root, DOUBLE_BOOKING), LOOSE);

    write(
        &root,
        DOUBLE_BOOKING,
        &LOOSE.replacen("  ping = GET", "  @after(begin)\n  ping = GET", 1),
    );
    let output = fix(&root);
    assert_eq!(output.status.code(), Some(0), "{}", both(&output));
    assert_eq!(read(&root, DOUBLE_BOOKING), LOOSE);
    let output = apiforge(
        &[
            "run",
            "--folder",
            "{root}",
            "--scenario",
            DOUBLE_BOOKING,
            "--json",
        ],
        &root,
    );
    assert_eq!(output.status.code(), Some(0), "{}", both(&output));
    let report: RunResult = serde_json::from_slice(&output.stdout).unwrap();
    let handles: Vec<&str> = report
        .steps
        .iter()
        .map(|step| step.handle.as_str())
        .collect();
    assert_eq!(handles, ["begin", "ping"]);
}

/// A working component whose first request reads a request written below
/// it is blocked, and both are named.
#[test]
fn lang20_criterion_3_a_component_reading_below_is_blocked() {
    let (_directory, root) = project();
    let text = "component sign_in \"Sign in\" {\n  @label(\"User\")\n  input user: string = \"tester\"\n  session = POST \"/sessions/{{probe.body.id}}\" {\n    body { user: param.user, password: \"secret\" }\n    sample 201 { token: \"tok-1\" }\n  }\n  probe = GET \"/probe\" {\n    sample 200 { id: \"p\" }\n  }\n  export token: string = session.body.token\n}\n";
    write(&root, SIGN_IN, text);
    let before = files(&root);
    let output = fix(&root);
    assert_eq!(output.status.code(), Some(1), "{}", both(&output));
    let report = stderr(&output);
    assert!(
        report.contains(&format!(
            "{SIGN_IN}:4:3: `session = POST \"/sessions/{{{{probe.body.id}}}}\" {{`: This statement is not written where it runs today: it runs after `probe`."
        )),
        "{report}"
    );
    assert_eq!(files(&root), before);
}

/// A cleanup whose first step reads a step written below it is blocked.
#[test]
fn lang20_criterion_3_a_cleanup_reading_below_is_blocked() {
    let (_directory, root) = project();
    let text = read(&root, DOUBLE_BOOKING).replacen(
        "  cleanup {\n",
        "  cleanup {\n    tidy = DELETE \"/tidy/{{listed.body.id}}\" {\n      sample 204 {}\n    }\n    listed = GET \"/tidy\" {\n      sample 200 { id: \"t\" }\n    }\n",
        1,
    );
    write(&root, DOUBLE_BOOKING, &text);
    let output = fix(&root);
    assert_eq!(output.status.code(), Some(1), "{}", both(&output));
    let report = stderr(&output);
    let tidy = line_of(&root, DOUBLE_BOOKING, "    tidy = DELETE");
    assert!(
        report.contains(&format!(
            "{DOUBLE_BOOKING}:{tidy}:5: `tidy = DELETE \"/tidy/{{{{listed.body.id}}}}\" {{`: This statement is not written where it runs today: it runs after `listed`."
        )),
        "{report}"
    );
    assert_eq!(language_of(&root), 1);
}

/// Rule 1: a source whose converted text would not pass the check a
/// language-2 project makes of it (here a scenario registered as a
/// component) blocks the project and is named.
#[test]
fn lang20_rule_1_a_result_that_would_not_pass_the_check_is_blocked() {
    let (_directory, root) = project();
    write(
        &root,
        SIGN_IN,
        LOOSE
            .replacen("  ping = GET", "  @after(begin)\n  ping = GET", 1)
            .as_str(),
    );
    let before = files(&root);
    let output = fix(&root);
    assert_eq!(output.status.code(), Some(1), "{}", both(&output));
    let report = stderr(&output);
    assert!(
        report.contains(&format!(
            "{SIGN_IN}: Once converted, the file would not pass the check: This code describes a different kind of document. Repair it."
        )),
        "{report}"
    );
    assert_eq!(files(&root), before);
}

// Criterion 4 ----------------------------------------------------------------

/// A source that is missing, does not parse or is over 1 MiB refuses the
/// project under its own heading and is named; the other sources are still
/// judged and reported.
#[test]
fn lang20_criterion_4_an_unreadable_source_is_named_under_its_heading() {
    for (unreadable, expected) in [
        ("missing", format!("{BOOKING_DAY}: The file is missing.")),
        ("syntax", format!("{BOOKING_DAY}:2:")),
        (
            "large",
            format!("{BOOKING_DAY}: The file is over the 1 MiB limit of a source."),
        ),
    ] {
        let (_directory, root) = project();
        block(&root);
        match unreadable {
            "missing" => fs::remove_file(root.join(BOOKING_DAY)).unwrap(),
            "syntax" => write(
                &root,
                BOOKING_DAY,
                "scenario \"Broken\" {\n  first = GET ]\n}\n",
            ),
            _ => {
                let mut text = read(&root, BOOKING_DAY);
                text.push_str(&format!("// {}\n", "x".repeat(1024 * 1024)));
                write(&root, BOOKING_DAY, &text);
            }
        }
        let before = files(&root);
        let output = fix(&root);
        assert_eq!(
            output.status.code(),
            Some(1),
            "{unreadable}: {}",
            both(&output)
        );
        let report = stderr(&output);
        let heading = report
            .find("Sources that cannot be read:")
            .unwrap_or_else(|| panic!("{unreadable}: {report}"));
        let named = report
            .find(&expected)
            .unwrap_or_else(|| panic!("{unreadable}: {expected}\n{report}"));
        assert!(heading < named, "{report}");
        assert!(report.contains("Statements to change by hand:"), "{report}");
        assert!(report.contains(&format!("{DOUBLE_BOOKING}:")), "{report}");
        assert!(report.contains(&format!("{NOTICE}:")), "{report}");
        assert_eq!(files(&root), before, "{unreadable}");
    }
}

/// A `.afs` file the manifest does not list is neither read nor changed.
#[test]
fn lang20_criterion_4_an_unlisted_source_is_not_the_projects() {
    let (_directory, root) = project();
    let stray = "scenario \"Stray\" {\n  begin = start\n  @after(begin)\n  ping = GET \"/ping\" {\n    sample 200 {}\n  }\n}\n";
    write(&root, "scenarios/stray.afs", stray);
    write(&root, "scenarios/broken.afs", "not a source {");
    let output = fix(&root);
    assert_eq!(output.status.code(), Some(0), "{}", both(&output));
    assert_eq!(read(&root, "scenarios/stray.afs"), stray);
    assert_eq!(read(&root, "scenarios/broken.afs"), "not a source {");
    assert!(!both(&output).contains("stray"), "{}", both(&output));
    assert!(!both(&output).contains("broken"), "{}", both(&output));
}

// Criterion 8 ----------------------------------------------------------------

/// `--check` prints the report of criterion 1 and writes nothing, local data
/// included, while another process holds the project; exit code 0.
#[test]
fn lang20_criterion_8_check_reports_a_convertible_project_and_writes_nothing() {
    let (_directory, root) = project();
    let holder = ProjectStore::open_exclusive_as(&root, &Holder::window()).unwrap();
    let before = tree(&root);
    let output = fix_check(&root);
    assert_eq!(output.status.code(), Some(0), "{}", both(&output));
    assert_eq!(
        tree(&root),
        before,
        "byte-identical, .apiforge/local included"
    );
    drop(holder);
    let listed = stdout(&output);
    let line = line_of(&root, BOOKING_DAY, "@after(begin)");
    assert!(
        listed.contains(&format!(
            "{BOOKING_DAY}:{line}: would remove `@after(begin)`"
        )),
        "{listed}"
    );
    assert!(listed.contains("nothing was written"), "{listed}");
    let (_other, fresh) = project();
    let written = stdout(&fix(&fresh));
    assert_eq!(
        listed
            .lines()
            .filter(|line| line.contains(".afs:"))
            .map(|line| line
                .replace("would remove", "removed")
                .replace("would write", "wrote"))
            .collect::<Vec<_>>(),
        written
            .lines()
            .filter(|line| line.contains(".afs:"))
            .map(str::to_owned)
            .collect::<Vec<_>>(),
        "the same report"
    );
}

/// `--check` of a blocked project: the report of criterion 2, exit code 1,
/// nothing written, also without a lock file.
#[test]
fn lang20_criterion_8_check_reports_a_blocked_project_and_writes_nothing() {
    let (_directory, root) = project();
    block(&root);
    let before = tree(&root);
    let output = fix_check(&root);
    assert_eq!(output.status.code(), Some(1), "{}", both(&output));
    assert_eq!(tree(&root), before);
    assert!(!root.join(".apiforge/local").exists());
    let report = stderr(&output);
    assert!(report.contains("Statements to change by hand:"), "{report}");
    assert!(report.contains(&format!("{NOTICE}:")), "{report}");
    assert!(report.contains(&format!("{DOUBLE_BOOKING}:")), "{report}");
}

/// A project already in language 2: nothing changes, exit code 0, one line.
#[test]
fn lang20_criterion_8_a_project_in_language_2_is_left_alone() {
    let (_directory, root) = project();
    assert_eq!(fix(&root).status.code(), Some(0));
    for check in [false, true] {
        let before = files(&root);
        let output = if check { fix_check(&root) } else { fix(&root) };
        assert_eq!(output.status.code(), Some(0), "{}", both(&output));
        assert_eq!(files(&root), before);
        assert_eq!(stdout(&output).lines().count(), 1, "{}", stdout(&output));
        assert!(stderr(&output).is_empty(), "{}", stderr(&output));
    }
}

/// A project of a newer language is refused with LANG-15's sentence, and
/// no lock file is created.
#[test]
fn lang20_criterion_8_a_newer_language_is_refused_before_any_lock() {
    let (_directory, root) = project();
    let path = root.join(MANIFEST_PATH);
    let mut manifest = ProjectManifest::from_bytes(&fs::read(&path).unwrap()).unwrap();
    (manifest.format_version, manifest.language_version) = (2, Some(3));
    fs::write(&path, manifest.to_public_bytes().unwrap()).unwrap();
    for check in [true, false] {
        let before = tree(&root);
        let output = if check { fix_check(&root) } else { fix(&root) };
        assert_eq!(output.status.code(), Some(1), "{}", both(&output));
        assert!(
            stderr(&output).contains(
                "This project is written in language 3; this build reads language 2. Update API Forge."
            ),
            "{}",
            stderr(&output)
        );
        assert_eq!(tree(&root), before);
        assert!(!root.join(".apiforge/local").exists());
    }
}

/// Without `--check`, a project held by an open window or an agent session
/// is refused as `apply` is, and nothing is written.
#[test]
fn lang20_criterion_8_a_held_project_is_refused_as_apply_is() {
    for holder in [
        Holder::window(),
        Holder::new(HolderRole::Agent, Some("Claude Code".into())),
    ] {
        let (_directory, root) = project();
        let owner = ProjectStore::open_exclusive_as(&root, &holder).unwrap();
        let before = files(&root);
        let output = fix(&root);
        assert_eq!(output.status.code(), Some(1), "{}", both(&output));
        assert!(stderr(&output).contains(HELD), "{}", stderr(&output));
        let replacement = root.with_file_name("replacement.afs");
        fs::write(&replacement, read(&root, DOUBLE_BOOKING)).unwrap();
        let apply = apiforge(
            &[
                "apply",
                "--folder",
                "{root}",
                "--file",
                DOUBLE_BOOKING,
                &replacement.display().to_string(),
            ],
            &root,
        );
        assert_eq!(stderr(&apply), stderr(&output), "refused as apply is");
        assert_eq!(files(&root), before);
        drop(owner);
    }
}

// Criterion 9 ----------------------------------------------------------------

/// No comment is lost: the comments of each converted source equal those
/// before; the published revision is byte-identical.
#[test]
fn lang20_criterion_9() {
    let (_directory, root) = project();
    let text = read(&root, BOOKING_DAY)
        .replacen(
            "  @after(begin)\n",
            "  // Sign in first.\n  @after(begin)\n",
            1,
        )
        .replacen(
            "  @after(big_enough)\n",
            "  @after(big_enough) // a team that exists\n",
            1,
        )
        .replacen(
            "  cleanup {\n",
            "  cleanup {\n    // Undo what the day booked.\n",
            1,
        );
    write(&root, BOOKING_DAY, &text);
    let text =
        read(&root, NOTICE).replacen("  @after(first)\n", "  @after(first) // once booked\n", 1);
    write(&root, NOTICE, &text);
    let before: BTreeMap<&str, Vec<String>> = SOURCES
        .iter()
        .map(|path| (*path, comments(&read(&root, path))))
        .collect();
    assert_eq!(before[BOOKING_DAY].len(), 3);
    let revision = fs::read(root.join(REVISION)).unwrap();
    let output = fix(&root);
    assert_eq!(output.status.code(), Some(0), "{}", both(&output));
    for path in SOURCES {
        let after = read(&root, path);
        assert!(!after.contains("@after"), "{path}");
        assert_eq!(comments(&after), before[path], "{path}");
    }
    assert_eq!(fs::read(root.join(REVISION)).unwrap(), revision);
}

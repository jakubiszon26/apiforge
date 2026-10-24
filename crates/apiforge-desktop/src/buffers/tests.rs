use super::*;
use apiforge_core::format::Project;

fn project() -> Project {
    apiforge_core::demos::shop_project()
}

fn context(project: &Project) -> Context {
    Context::Scenario {
        project: project.id.clone(),
        id: project.scenarios[0].id.clone(),
    }
}

fn document(project: &Project) -> Document {
    Document::Scenario(project.scenarios[0].clone())
}

/// A moment far enough in the past that the pause before applying is over.
fn settled() -> (Instant, Instant) {
    let typed = Instant::now();
    (typed, typed + APPLY_DELAY + Duration::from_millis(1))
}

#[test]
fn a_clean_buffer_is_whatever_the_document_prints() {
    let project = project();
    let context = context(&project);
    let printed = print(&project, &document(&project)).expect("the demo prints");
    let mut buffers = Buffers::default();
    let buffer = buffers
        .text(&context, Some(printed.clone()), 0, false)
        .unwrap();
    assert_eq!(buffer.text, printed);
    assert_eq!(buffer.state, State::Clean);
    // Nothing is kept for a clean buffer, so a document edit simply reprints.
    assert!(buffers.drafts().next().is_none());
}

#[test]
fn text_becomes_a_document_only_after_typing_pauses() {
    let project = project();
    let context = context(&project);
    let printed = print(&project, &document(&project)).unwrap();
    let mut buffers = Buffers::default();
    buffers.text(&context, Some(printed.clone()), 0, false);
    let (typed, later) = settled();
    buffers.typed(
        &context,
        printed.replace("scenario \"", "scenario \"New "),
        typed,
    );
    // Still typing: nothing is applied yet.
    assert!(matches!(
        buffers.apply(&context, &project, &document(&project), typed),
        Apply::Nothing
    ));
    let applied = buffers.apply(&context, &project, &document(&project), later);
    let Apply::Scenario(scenario) = applied else {
        panic!("a paused valid buffer applies: {applied:?}");
    };
    assert!(scenario.name.starts_with("New "));
    assert_eq!(buffers.get(&context).unwrap().state, State::Applied);
    // Applying once is enough; the same text does not apply again.
    assert!(matches!(
        buffers.apply(&context, &project, &document(&project), later),
        Apply::Nothing
    ));
}

#[test]
fn invalid_text_keeps_the_document_and_holds_the_board() {
    let project = project();
    let context = context(&project);
    let printed = print(&project, &document(&project)).unwrap();
    let mut buffers = Buffers::default();
    buffers.text(&context, Some(printed.clone()), 0, false);
    let (typed, later) = settled();
    buffers.typed(&context, format!("{printed}\n  oops ="), typed);
    assert!(matches!(
        buffers.apply(&context, &project, &document(&project), later),
        Apply::Failed
    ));
    let buffer = buffers.get(&context).unwrap();
    assert_eq!(buffer.state, State::Invalid);
    assert!(buffer.errors().next().is_some(), "the problem is located");
    assert!(buffers.blocks_the_board(&context));
    // An invalid buffer is never replaced by a reprint: the text is not lost.
    let kept = buffers
        .text(&context, Some(printed.clone()), 0, false)
        .unwrap()
        .text
        .clone();
    assert!(kept.ends_with("oops ="), "{kept}");
}

#[test]
fn discarding_goes_back_to_the_document() {
    let project = project();
    let context = context(&project);
    let printed = print(&project, &document(&project)).unwrap();
    let mut buffers = Buffers::default();
    let (typed, later) = settled();
    buffers.typed(&context, "nonsense".to_owned(), typed);
    buffers.apply(&context, &project, &document(&project), later);
    assert!(buffers.blocks_the_board(&context));
    buffers.discard(&context);
    assert!(!buffers.blocks_the_board(&context));
    assert_eq!(
        buffers
            .text(&context, Some(printed.clone()), 0, false)
            .unwrap()
            .text,
        printed
    );
}

#[test]
fn an_applied_buffer_keeps_its_spelling_until_the_document_moves_on() {
    let project = project();
    let context = context(&project);
    let printed = print(&project, &document(&project)).unwrap();
    let spaced = printed.replace("\n}", "\n\n}");
    let mut buffers = Buffers::default();
    let (typed, later) = settled();
    buffers.typed(&context, spaced.clone(), typed);
    buffers.apply(&context, &project, &document(&project), later);
    assert_eq!(buffers.get(&context).unwrap().state, State::Applied);
    buffers.applied(&context, 1);

    // The document is where this text put it, so the spelling stands.
    assert_eq!(
        buffers
            .text(&context, Some(printed.clone()), 1, false)
            .unwrap()
            .text,
        spaced
    );
    // The caret is in it, so it is not reformatted underneath.
    assert_eq!(
        buffers
            .text(&context, Some(printed.clone()), 2, true)
            .unwrap()
            .text,
        spaced
    );
    // The document moved on without it, so the panel shows what it now says.
    assert_eq!(
        buffers
            .text(&context, Some(printed.clone()), 2, false)
            .unwrap()
            .text,
        printed
    );
    assert_eq!(buffers.get(&context).unwrap().state, State::Clean);
}

#[test]
fn a_draft_is_every_buffer_the_document_does_not_already_say() {
    let project = project();
    let context = context(&project);
    let printed = print(&project, &document(&project)).unwrap();
    let mut buffers = Buffers::default();
    buffers.text(&context, Some(printed.clone()), 0, false);
    assert_eq!(buffers.drafts().count(), 0);
    let (typed, later) = settled();
    buffers.typed(&context, format!("{printed}\n  broken ="), typed);
    buffers.apply(&context, &project, &document(&project), later);
    assert_eq!(buffers.drafts().count(), 1);
}

#[test]
fn code_for_another_document_is_refused_rather_than_applied() {
    let project = project();
    let context = context(&project);
    let mut buffers = Buffers::default();
    let (typed, later) = settled();
    // Valid code, but it declares a component where a scenario is expected.
    buffers.typed(
        &context,
        "component helper \"Helper\" {\n  begin = start\n}".to_owned(),
        typed,
    );
    let applied = buffers.apply(&context, &project, &document(&project), later);
    assert!(matches!(applied, Apply::Failed), "{applied:?}");
    let buffer = buffers.get(&context).unwrap();
    assert_eq!(buffer.state, State::Invalid);
    assert!(
        buffer
            .errors()
            .any(|error| error.message.contains("different document")),
        "the reason is stated"
    );
}

#[test]
fn a_document_that_cannot_be_written_as_code_produces_no_text() {
    let project = project();
    let context = context(&project);
    let mut buffers = Buffers::default();
    assert!(buffers.text(&context, None, 0, false).is_none());
}

#[test]
fn a_restored_draft_waits_to_be_parsed_and_can_be_stale() {
    let project = project();
    let context = context(&project);
    let mut buffers = Buffers::default();
    let printed = print(&project, &document(&project)).unwrap();
    buffers.restore(
        context.clone(),
        Draft {
            text: printed.clone(),
            cursor: 4,
            stale: true,
        },
    );
    let buffer = buffers.get(&context).unwrap();
    assert_eq!(buffer.cursor, 4);
    assert!(buffer.stale);
    // A stale draft stays unapplied until the user answers the conflict.
    let (_, later) = settled();
    let applied = buffers.apply(&context, &project, &document(&project), later);
    assert!(
        matches!(applied, Apply::Nothing),
        "an unresolved draft cannot change the document: {applied:?}"
    );
    // Typing answers the question the stale banner asks.
    buffers.typed(&context, format!("{printed}\n"), Instant::now());
    assert!(!buffers.get(&context).unwrap().stale);
}

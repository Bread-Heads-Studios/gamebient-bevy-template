//! The sponsor-kit starter pack is complete except for its TODO: fields.

use gamebient_game::look::validate::validate;
use gamebient_game::look::{Severity, pack};
use gamebient_game::ui::theme;

#[test]
fn the_starter_fails_only_on_its_todo_placeholders() {
    let files =
        pack::read_dir(std::path::Path::new("docs/sponsor-kit/starter")).expect("starter reads");
    let r = validate(&files, &theme::LOOK, Some("starter"));
    let errors: Vec<_> = r
        .problems
        .iter()
        .filter(|p| p.severity == Severity::Error)
        .collect();
    assert!(!errors.is_empty(), "the starter must not load as-is");
    for e in &errors {
        assert!(
            e.message.contains("TODO:"),
            "non-TODO error: {} {}",
            e.at,
            e.message
        );
    }
    assert!(errors.iter().any(|e| e.at == "end_card.headline"));
    assert!(
        r.problems.iter().all(|p| p.severity == Severity::Error),
        "no warnings: {:#?}",
        r.problems
    );
}

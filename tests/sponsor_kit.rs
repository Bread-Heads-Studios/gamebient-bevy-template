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

/// The end-card sample sets nothing a game's contract could lack, so it
/// loads in any game that declares the four `endcard*` roles, which is
/// every game that shows a sponsor end card. Ports check their card with it.
#[test]
fn the_end_card_sample_validates_against_an_end_card_only_contract() {
    use bevy::color::Color;
    use gamebient_game::look::Contract;

    static END_CARD_ONLY: Contract = Contract {
        palette: &[
            ("endcard", Color::srgba(0.0, 0.0, 0.0, 0.0)),
            ("endcardhead", Color::WHITE),
            ("endcardbody", Color::WHITE),
            ("endcardurl", Color::WHITE),
        ],
        strings: &[],
        slots: &[],
        audio: &[],
        fonts: &[],
        glow: &[],
    };
    let path = std::path::Path::new("docs/sponsor-kit/end-card-sample");
    let files = pack::read_dir(path).expect("sample reads");
    assert_eq!(pack::dir_id(path).as_deref(), Some("end-card-sample"));
    for contract in [&END_CARD_ONLY, &theme::LOOK] {
        let r = validate(&files, contract, Some("end-card-sample"));
        assert!(r.problems.is_empty(), "{:#?}", r.problems);
        let card = r.manifest.and_then(|m| m.end_card).expect("an end card");
        assert_eq!(card.image.as_deref(), Some("logo.png"));
    }
}

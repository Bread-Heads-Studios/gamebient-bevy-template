//! The demo bundle's content cap. TEMPLATE NOTE: a port changes `DEMO_CAP`
//! and `demo_reached` (and nothing else here) to the game's own progression:
//! "after level 2", "after hole 3", "after the first song". The cut fires the
//! moment the capped unit is complete; `end_demo` then ends the run exactly
//! the way `sim::end_run` does, through the fade into `GameState::DemoEnd`.
//!
//! Compiled in only with `--features demo`. The sim, seed and replay code
//! never see the flag: a demo run and a full run are identical up to the cut.

use bevy::prelude::*;
use gamebient_input::HostEvent;

use crate::game::scoring::GameData;
use crate::game::sim;
use crate::game::states::GameState;
use crate::ui::transition::ScreenFade;

/// True in the demo bundle. UI reads it to tag the title card.
pub const DEMO: bool = cfg!(feature = "demo");

/// TEMPLATE NOTE: the template has no levels, so its demo ends at a score.
/// A port replaces this with its own unit ("after level 2": `level >= 3`).
pub const DEMO_CAP: u32 = 1_000;

/// Present on apps that must never cut a run short: the headless replay
/// apps (`replay::build_headless_app`) replay whole runs, and the demo cut
/// is a UI-layer concern. The demo's own tests build plain headless apps
/// without it, so they still exercise the cut.
#[derive(Resource, Default)]
pub struct DemoCutOff;

/// The cut, as a pure function of whatever the game already tracks.
/// TEMPLATE NOTE: a port reads its own progression resource instead of
/// `GameData` (change the `end_demo` parameter to match).
pub fn demo_reached(data: &GameData) -> bool {
    data.score >= DEMO_CAP
}

/// Ends the run at the cut: latches `RunOver` so this is the last simulated
/// tick (the sim freezes, like `sim::end_run`), tells the host, and leaves
/// `Playing` through the fade when there is one, or `NextState` when there
/// is not. Registered in `FixedUpdate` after `SimSet`, demo feature only.
pub fn end_demo(
    data: Res<GameData>,
    mut over: ResMut<sim::RunOver>,
    fade: Option<ResMut<ScreenFade>>,
    mut next: ResMut<NextState<GameState>>,
    mut out: MessageWriter<HostEvent>,
) {
    if over.0 || !demo_reached(&data) {
        return;
    }
    over.0 = true;
    out.write(HostEvent::Custom {
        name: "demo_end".into(),
        data: "{}".into(),
    });
    match fade {
        Some(mut fade) => {
            fade.request(GameState::DemoEnd);
        }
        None => next.set(GameState::DemoEnd),
    }
}

#[cfg(test)]
mod tests {
    use bevy::input::InputPlugin;
    use bevy::state::app::StatesPlugin;
    use bevy::time::TimeUpdateStrategy;
    use gamebient_input::HostEvent;

    use super::*;
    use crate::game::GamePlugin;
    use crate::game::scoring::GameData;
    use crate::game::sim;
    use crate::game::states::GameState;

    #[test]
    fn cut_is_reached_at_the_cap_and_not_before() {
        let below = GameData {
            score: DEMO_CAP - 1,
            ..Default::default()
        };
        assert!(!demo_reached(&below));
        let at = GameData {
            score: DEMO_CAP,
            ..Default::default()
        };
        assert!(demo_reached(&at));
    }

    #[cfg(feature = "demo")]
    #[test]
    fn reaching_the_cap_ends_the_run_into_demo_end_and_tells_the_host() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_plugins(StatesPlugin)
            .add_plugins(InputPlugin)
            .insert_resource(TimeUpdateStrategy::ManualDuration(sim::tick_duration()))
            .add_plugins(GamePlugin { headless: true });
        app.update();
        app.world_mut()
            .resource_mut::<NextState<GameState>>()
            .set(GameState::Playing);
        app.update();
        app.update(); // one fixed tick under the manual clock
        assert!(!app.world().resource::<sim::RunOver>().0);

        app.world_mut().resource_mut::<GameData>().score = DEMO_CAP;
        app.update(); // end_demo runs after SimSet, latches RunOver, requests DemoEnd
        assert!(app.world().resource::<sim::RunOver>().0);
        app.update(); // the transition applies
        assert_eq!(
            *app.world().resource::<State<GameState>>().get(),
            GameState::DemoEnd
        );

        let events = app.world().resource::<Messages<HostEvent>>();
        let mut cursor = events.get_cursor();
        let custom: Vec<_> = cursor
            .read(events)
            .filter(|e| matches!(e, HostEvent::Custom { name, .. } if name == "demo_end"))
            .cloned()
            .collect();
        assert_eq!(custom.len(), 1, "exactly one demo_end event");
        assert!(matches!(&custom[0], HostEvent::Custom { data, .. } if data == "{}"));
    }

    #[cfg(feature = "demo")]
    #[test]
    fn demo_cut_off_keeps_the_run_going_past_the_cap() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_plugins(StatesPlugin)
            .add_plugins(InputPlugin)
            .insert_resource(TimeUpdateStrategy::ManualDuration(sim::tick_duration()))
            .insert_resource(DemoCutOff)
            .add_plugins(GamePlugin { headless: true });
        app.update();
        app.world_mut()
            .resource_mut::<NextState<GameState>>()
            .set(GameState::Playing);
        app.update();
        app.update();
        app.world_mut().resource_mut::<GameData>().score = DEMO_CAP + 50;
        let mut cursor = app.world().resource::<Messages<HostEvent>>().get_cursor();
        let mut n = 0;
        for _ in 0..5 {
            app.update();
            let events = app.world().resource::<Messages<HostEvent>>();
            n += cursor
                .read(events)
                .filter(|e| matches!(e, HostEvent::Custom { name, .. } if name == "demo_end"))
                .count();
        }
        assert_eq!(n, 0, "no demo_end event");
        assert!(!app.world().resource::<sim::RunOver>().0);
        assert_eq!(
            *app.world().resource::<State<GameState>>().get(),
            GameState::Playing
        );
    }

    #[cfg(feature = "demo")]
    #[test]
    fn the_cut_fires_once_even_if_the_score_stays_above_the_cap() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_plugins(StatesPlugin)
            .add_plugins(InputPlugin)
            .insert_resource(TimeUpdateStrategy::ManualDuration(sim::tick_duration()))
            .add_plugins(GamePlugin { headless: true });
        app.update();
        app.world_mut()
            .resource_mut::<NextState<GameState>>()
            .set(GameState::Playing);
        app.update();
        app.world_mut().resource_mut::<GameData>().score = DEMO_CAP + 50;
        // Messages double-buffer away after two updates, so count per update.
        let mut cursor = app.world().resource::<Messages<HostEvent>>().get_cursor();
        let mut n = 0;
        for _ in 0..5 {
            app.update();
            let events = app.world().resource::<Messages<HostEvent>>();
            n += cursor
                .read(events)
                .filter(|e| matches!(e, HostEvent::Custom { name, .. } if name == "demo_end"))
                .count();
        }
        assert_eq!(n, 1);
    }
}

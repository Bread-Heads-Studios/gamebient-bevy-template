//! A live song edit leaks only genuinely new data: identical `songs.json`
//! re-sends reuse the first leak (`content::audio`'s intern table). One
//! test in its own binary, so nothing else touches the table meanwhile.

use bevy::prelude::*;
use gamebient_game::game::audio::{CurrentCue, MusicCache, SfxEvent, setup_sfx};
use gamebient_game::game::content::audio::{
    SongOverrides, intern_counts, load_embedded_audio, set_songs,
};

fn songs_file(bpm: u32) -> String {
    format!(
        r#"{{"v":1,"songs":{{"menu":{{"bpm":{bpm},"beats_per_bar":4,"steps_per_beat":4,"bars":1,
        "voices":["c4:8 e4:8","g3:16",".:16"],"noise":".:16","instruments":[{{"env":[0,4,8]}}]}}}}}}"#
    )
}

fn first_voice(world: &World) -> *const u8 {
    world.resource::<SongOverrides>().0[0].1.voices[0].as_ptr()
}

#[test]
fn identical_songs_leak_once_and_each_new_version_once() {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, AssetPlugin::default()))
        .init_asset::<AudioSource>()
        .add_message::<SfxEvent>()
        .init_resource::<MusicCache>()
        .init_resource::<CurrentCue>()
        .add_systems(Startup, (load_embedded_audio, setup_sfx).chain());
    app.update();
    let w = app.world_mut();
    let before = intern_counts();

    set_songs(w, songs_file(120).as_bytes()).unwrap();
    let once = intern_counts();
    assert_eq!(once.songs, before.songs + 1);
    let first = first_voice(w);
    for _ in 0..1000 {
        set_songs(w, songs_file(120).as_bytes()).unwrap();
    }
    assert_eq!(
        intern_counts(),
        once,
        "1000 re-sends of the same file leak nothing new"
    );
    assert_eq!(first_voice(w), first, "the same leaked data is reused");

    for n in 0..10 {
        set_songs(w, songs_file(130 + n).as_bytes()).unwrap();
    }
    let after = intern_counts();
    assert_eq!(
        after.songs,
        once.songs + 10,
        "10 distinct versions, 10 interned songs"
    );
    // Only the tempo changed: the strings, envelopes and tables are shared.
    assert_eq!(after.strings, once.strings);
    assert_eq!(after.envelopes, once.envelopes);
    assert_eq!(after.tables, once.tables);
}

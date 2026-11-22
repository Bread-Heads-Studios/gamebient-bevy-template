# End-card sample

A sponsor pack with an end card and nothing else: no palette, strings or
slots, so every game's contract accepts it. A port uses it to check its
end card:

    cargo run -- --look <template>/docs/sponsor-kit/end-card-sample

It is not a built-in pack. Nothing builds, embeds or ships `docs/`; never
copy it into a game's `assets/looks/`.

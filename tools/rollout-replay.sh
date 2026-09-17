#!/usr/bin/env bash
# Copies replay verification (deterministic sim, GXR1 replays, the wasm
# verifier: src/game/sim.rs, src/game/replay/, src/bin/verify.rs, build.rs,
# tools/build_verify.sh, tools/verify_fixture.mjs, docs/replay-verification.md)
# from this template into a game checkout and wires it: Cargo.toml lib/bin
# split + features + deps, src/lib.rs + src/main.rs, the `pub mod
# replay`/`sim` + `GamePlugin { headless }` shape in src/game/mod.rs,
# build_web.sh's verify.zip step, the CI/release workflow steps, .gitignore,
# and assets/info.json's verify_url. Idempotent. The script never edits a
# copied file's contents (the two exceptions — src/bin/verify.rs and
# tests/selftest.rs — get only a mechanical `gamebient_game::` ->
# `<snake>::` crate-path substitution; see the comment at that copy step).
# game-specific behaviour (porting GamePlugin::build onto sim::SimSet,
# adding this game's own checksum_<game> system after sim::checksum_tick,
# writing the selftest script) is a HAND EDIT for the skill's
# port-checklist, not this script.
#
# Usage: tools/rollout-replay.sh <game-dir>
set -euo pipefail
TEMPLATE="$(cd "$(dirname "$0")/.." && pwd)"
GAME="$(cd "${1:?usage: $0 <game-dir>}" && pwd)"
PKG=$(grep -m1 '^name' "$GAME/Cargo.toml" | sed -E 's/.*"([^"]+)".*/\1/')
SNAKE=${PKG//-/_}

# ---------------------------------------------------------------------------
# Pre-flight HAND EDITs. Informational only — never stop the script.
# ---------------------------------------------------------------------------
if [ ! -f "$GAME/src/game/mod.rs" ]; then
  echo "HAND EDIT: src/game/mod.rs: flat layout (no src/game/); wire sim/replay and GamePlugin by hand, see irregular-games.md"
fi
if [ ! -d "$GAME/src/game/record" ]; then
  echo "HAND EDIT: src/game/record/ missing; run tools/rollout-record.sh first"
fi
if [ -e "$GAME/src/game/sim.rs" ] && ! cmp -s "$TEMPLATE/src/game/sim.rs" "$GAME/src/game/sim.rs"; then
  echo "HAND EDIT: src/game/sim.rs: already exists and isn't the template's sim.rs; rename your sim.rs (e.g. shot_sim.rs) and re-run"
fi
if [ -f "$GAME/src/game/scoring.rs" ] && ! grep -q 'pub score' "$GAME/src/game/scoring.rs"; then
  echo "HAND EDIT: src/game/scoring.rs: no 'score' field on GameData; implement LeaderboardScore for it"
fi
if grep -rn "ButtonInput<KeyCode>" "$GAME/src/game" --include="*.rs" 2>/dev/null | grep -v autopilot | grep -q .; then
  echo "HAND EDIT: src/game: gameplay reads raw ButtonInput<KeyCode>; route through TickInput instead"
fi
if [ -f "$GAME/src/game/autopilot.rs" ] && grep -n 'data\.\w* = ' "$GAME/src/game/autopilot.rs" | grep -q .; then
  echo "HAND EDIT: src/game/autopilot.rs: writes GameData fields directly (dev-only; keep that path out of the selftest script)"
fi
if [ -f "$GAME/src/game/host.rs" ] && grep -q 'match command' "$GAME/src/game/host.rs" \
  && ! grep -q 'HostCommand::Seed' "$GAME/src/game/host.rs" \
  && ! grep -qE '^\s*_\s*=>' "$GAME/src/game/host.rs"; then
  echo "HAND EDIT: src/game/host.rs: gamebient-input v0.3.0 added HostCommand::Seed; add 'HostCommand::Seed(bytes) => pending.0 = Some(*bytes)' (needs a 'mut pending: ResMut<sim::PendingSeed>' param) to apply_host_commands so host-supplied seeds reach the sim, or the match won't compile"
fi

# ---------------------------------------------------------------------------
# Copy the feature files verbatim. A file that already exists and isn't
# byte-identical to the template's is left alone (HAND EDIT), never
# overwritten.
# ---------------------------------------------------------------------------
mkdir -p "$GAME/src/game/replay" "$GAME/src/bin" "$GAME/tests/fixtures" "$GAME/tools"

copy_or_hand_edit() {
  local rel="$1" src dst
  src="$TEMPLATE/$rel"
  dst="$GAME/$rel"
  if [ -e "$dst" ]; then
    if ! cmp -s "$src" "$dst"; then
      echo "HAND EDIT: $rel: exists and differs from the template's version; move it aside and re-run to pick up the template copy"
    fi
  else
    mkdir -p "$(dirname "$dst")"
    cp "$src" "$dst"
  fi
}

# sim.rs already has its own pre-flight message (rename-and-re-run) above;
# only perform the copy here, and only when nothing is in the way.
[ -e "$GAME/src/game/sim.rs" ] || cp "$TEMPLATE/src/game/sim.rs" "$GAME/src/game/sim.rs"

for rel in \
  src/game/replay/mod.rs \
  src/game/replay/recorder.rs \
  src/game/replay/feeder.rs \
  src/game/replay/selftest.rs \
  build.rs \
  tools/build_verify.sh \
  tools/verify_fixture.mjs \
  docs/replay-verification.md \
  ; do
  copy_or_hand_edit "$rel"
done
chmod +x "$GAME/tools/build_verify.sh" 2>/dev/null || true

# src/bin/verify.rs and tests/selftest.rs hardcode `use gamebient_game::...`
# (the template's own crate name). Cargo has no self-dependency aliasing
# that would let `gamebient_game::` resolve to this game's lib crate
# (verified empirically: a `path = "."` self-dependency is a cyclic-package
# error under [dependencies], and dev-dependencies aren't linked into plain
# `cargo build`/`cargo check` bin targets either) — so a byte-identical copy
# cannot compile once [lib].name is this game's own <snake>. Copy these two
# files with that one mechanical substitution; nothing else about their
# content changes. On the template itself SNAKE == "gamebient_game", so the
# substitution is a no-op and the file stays byte-identical (still a no-op
# rerun on the template's own checkout).
copy_with_crate_rename() {
  local rel="$1" src dst expected
  src="$TEMPLATE/$rel"
  dst="$GAME/$rel"
  expected="$(sed "s/gamebient_game::/${SNAKE}::/g" "$src")"
  if [ -e "$dst" ]; then
    if [ "$(cat "$dst")" != "$expected" ]; then
      echo "HAND EDIT: $rel: exists and differs from the template's version (with gamebient_game:: -> ${SNAKE}::); move it aside and re-run"
    fi
  else
    mkdir -p "$(dirname "$dst")"
    printf '%s\n' "$expected" >"$dst"
  fi
}
copy_with_crate_rename src/bin/verify.rs
copy_with_crate_rename tests/selftest.rs

# ---------------------------------------------------------------------------
# Wiring edits: Cargo.toml, src/lib.rs + src/main.rs, src/game/mod.rs,
# build_web.sh, CI/release workflows, .gitignore, assets/info.json. One perl
# pass so every anchor/guard lives in one place; each edit is guarded so a
# second run is a no-op, and every miss prints a HAND EDIT instead of
# guessing.
# ---------------------------------------------------------------------------
perl - "$GAME" "$PKG" "$SNAKE" <<'PERL_EOF'
use strict;
use warnings;

my ($game, $pkg, $snake) = @ARGV;
my @hand_edits;
sub hand_edit { push @hand_edits, "HAND EDIT: $_[0]"; }

sub slurp {
    my $p = shift;
    return undef unless -e $p;
    open my $fh, '<', $p or die "read $p: $!";
    local $/;
    my $c = <$fh>;
    close $fh;
    return $c;
}
sub spit {
    my ($p, $c) = @_;
    open my $fh, '>', $p or die "write $p: $!";
    print $fh $c;
    close $fh;
}

# ---------- Cargo.toml ----------
{
    my $path = "$game/Cargo.toml";
    my $c = slurp($path);
    if (defined $c) {
        my $orig = $c;

        if ($c !~ /^\[lib\]/m) {
            my $block = "\n[lib]\nname = \"$snake\"\npath = \"src/lib.rs\"\n\n[[bin]]\nname = \"$pkg\"\npath = \"src/main.rs\"\n\n[[bin]]\nname = \"verify\"\npath = \"src/bin/verify.rs\"\nrequired-features = [\"verify\"]\n";
            unless ($c =~ s/(edition = "2024"\n)/$1$block/) {
                hand_edit("Cargo.toml: no 'edition = \"2024\"' line to anchor the [lib]/[[bin]] blocks on; add them by hand");
            }
        }

        if ($c !~ /^verify = /m) {
            my $block = "# Replay verifier entry point (src/bin/verify.rs): native CLI and the\n# wasm-bindgen module the site runs under Node. See docs/replay-verification.md.\nverify = [\"dep:wasm-bindgen\"]\n";
            unless ($c =~ s/(^record = \[.*\]\n)/$1$block/m) {
                hand_edit('Cargo.toml: add \'verify = ["dep:wasm-bindgen"]\' under [features], after record');
            }
        }

        if ($c !~ /^rand_xoshiro = /m) {
            my $block = "# Sim RNG: Xoshiro256++ explicitly, never SmallRng (which picks a different\n# algorithm on wasm32 and would break native-vs-wasm replay determinism).\nrand_xoshiro = \"0.7\"\n";
            unless ($c =~ s/(^rand = "0\.9"\n)/$1$block/m) {
                hand_edit('Cargo.toml: add rand_xoshiro = "0.7" after rand = "0.9"');
            }
        }

        if ($c !~ /^wasm-bindgen = /m) {
            my $line = "wasm-bindgen = { version = \"0.2.108\", optional = true }\n";
            unless ($c =~ s/(^getrandom = \{ version = "0\.3", features = \["wasm_js"\] \}\n)/$1$line/m) {
                hand_edit('Cargo.toml: add wasm-bindgen = { version = "0.2.108", optional = true } after the wasm32 getrandom line');
            }
        }

        if ($c =~ /gamebient-input = .*tag = "v0\.2\.\d+"/) {
            $c =~ s/(gamebient-input = .*tag = ")v0\.2\.\d+(")/$1v0.3.0$2/;
        }
        if ($c !~ /gamebient-input = .*tag = "v0\.3\.0"/) {
            hand_edit('Cargo.toml: pin gamebient-input tag = "v0.3.0"');
        }

        spit($path, $c) if $c ne $orig;
    } else {
        hand_edit("Cargo.toml not found");
    }
}

# ---------- src/lib.rs + src/main.rs ----------
{
    my $lib_path = "$game/src/lib.rs";
    my $main_path = "$game/src/main.rs";
    my $main_c = slurp($main_path);
    if (defined $main_c) {
        my $orig_main = $main_c;
        my $has_gameplugin = ($main_c =~ /game::GamePlugin\b/) ? 1 : 0;

        # Keep line endings on each element so re-joining is exact.
        my @lines = split /(?<=\n)/, $main_c;
        my @out;
        my @plain;
        my @cfg; # [ "#[cfg(...)]\n", "modname" ]

        for my $line (@lines) {
            if ($line =~ /^mod (\w+);\s*\n?$/) {
                my $name = $1;
                if (@out && $out[-1] =~ /^#\[cfg\([^)]*\)\]\s*\n?$/) {
                    my $attr = pop @out;
                    push @cfg, [$attr, $name];
                } else {
                    push @plain, $name;
                }
                next;
            }
            push @out, $line;
        }

        if (@plain || @cfg) {
            unless (-e $lib_path) {
                my $lib_c = "#![allow(clippy::too_many_arguments, clippy::type_complexity)]\n\n";
                $lib_c .= "pub mod $_;\n" for @plain;
                $lib_c .= "$_->[0]pub mod $_->[1];\n" for @cfg;
                spit($lib_path, $lib_c);
            }

            my $use_line = "use $snake" . "::{" . join(", ", @plain) . "};\n";
            my @cfg_use = map { "$_->[0]" . "use $snake" . "::$_->[1];\n" } @cfg;

            my $insert_at = 0;
            for my $i (0 .. $#out) {
                if ($out[$i] =~ /^use /) { $insert_at = $i; last; }
            }
            splice(@out, $insert_at, 0, $use_line, @cfg_use);
        }

        $main_c = join('', @out);
        if ($has_gameplugin) {
            $main_c =~ s/\bgame::GamePlugin\b(?!::default\(\))/game::GamePlugin::default()/g;
        } else {
            hand_edit("src/main.rs: no literal 'game::GamePlugin' found; wire game::GamePlugin::default() by hand");
        }

        spit($main_path, $main_c) if $main_c ne $orig_main;
    } else {
        hand_edit("no src/main.rs found");
    }
}

# ---------- src/game/mod.rs ----------
{
    my $path = "$game/src/game/mod.rs";
    my $c = slurp($path);
    if (defined $c) {
        my $orig = $c;

        if ($c !~ /^pub mod replay;/m) {
            unless ($c =~ s/^pub mod scoring;\n/pub mod replay;\npub mod scoring;\n/m) {
                hand_edit("src/game/mod.rs: declare 'pub mod replay;' (alphabetically, next to the other pub mod lines)");
            }
        }
        if ($c !~ /^pub mod sim;/m) {
            unless ($c =~ s/^pub mod scoring;\n/pub mod scoring;\npub mod sim;\n/m) {
                hand_edit("src/game/mod.rs: declare 'pub mod sim;' (alphabetically, next to the other pub mod lines)");
            }
        }

        if ($c !~ /pub headless: bool/) {
            my $repl = "#[derive(Default)]\npub struct GamePlugin {\n    pub headless: bool,\n}\n";
            unless ($c =~ s/^pub struct GamePlugin;\s*?\n/$repl/m) {
                hand_edit('src/game/mod.rs: replace \'pub struct GamePlugin;\' with \'#[derive(Default)] pub struct GamePlugin { pub headless: bool }\'');
            }
        }

        # Always surfaced: the build() body is game-specific free-form code
        # this script cannot safely touch (system ordering, what stays in
        # Update vs moves into sim::SimSet, what this game's own checksum
        # system folds).
        hand_edit('src/game/mod.rs: port GamePlugin::build to run gameplay through sim::SimSet, gate scene/audio/dev-harness setup on !self.headless, and add a checksum_<game> system after sim::checksum_tick folding your key run state (see docs/replay-verification.md rule 6)');

        spit($path, $c) if $c ne $orig;
    } else {
        hand_edit("no src/game/mod.rs found (flat layout?); declare 'pub mod replay'/'pub mod sim' and port GamePlugin by hand");
    }
}

# ---------- build_web.sh ----------
{
    my $path = "$game/build_web.sh";
    my $c = slurp($path);
    if (defined $c) {
        my $orig = $c;

        if ($c !~ /! -name 'verify\.wasm'/) {
            my $old = "WASM=\$(find target/wasm32-unknown-unknown/wasm-release -maxdepth 1 -name '*.wasm' | head -1)\n"
                    . "if [ -z \"\$WASM\" ]; then\n"
                    . "    echo \"ERROR: no .wasm found in target/wasm32-unknown-unknown/wasm-release/\" >&2\n"
                    . "    exit 1\n"
                    . "fi\n";
            my $new = "WASM=\$(find target/wasm32-unknown-unknown/wasm-release -maxdepth 1 -name '*.wasm' ! -name 'verify.wasm')\n"
                    . "if [ -z \"\$WASM\" ]; then\n"
                    . "    echo \"ERROR: no game .wasm found in target/wasm32-unknown-unknown/wasm-release/\" >&2\n"
                    . "    exit 1\n"
                    . "fi\n"
                    . "if [ \"\$(printf '%s\\n' \"\$WASM\" | wc -l | tr -d ' ')\" -ne 1 ]; then\n"
                    . "    echo \"ERROR: more than one candidate .wasm in target/wasm32-unknown-unknown/wasm-release/:\" >&2\n"
                    . "    printf '%s\\n' \"\$WASM\" >&2\n"
                    . "    echo \"Remove the stale ones (or 'cargo clean') so the deployed bundle is unambiguous.\" >&2\n"
                    . "    exit 1\n"
                    . "fi\n";
            my $idx = index($c, $old);
            if ($idx >= 0) {
                substr($c, $idx, length($old)) = $new;
            } else {
                hand_edit("build_web.sh: the 'find ... *.wasm | head -1' block doesn't match the template's; add the verify.wasm exclusion by hand");
            }
        }

        if ($c !~ /tools\/build_verify\.sh/) {
            my $anchor = "    dist/${pkg}_bg.wasm -o dist/${pkg}_bg.wasm\n";
            my $addition = "\n# Build the headless replay verifier and publish it alongside the game bundle\n"
                          . "# as dist/verify.zip — the site fetches it from properties.verify_url. Run\n"
                          . "# after this script's own wasm-bindgen/wasm-opt steps (and after the `find`\n"
                          . "# above, which already excludes verify.wasm by name) so there's no ambiguity\n"
                          . "# about which .wasm is the game bundle.\n"
                          . "bash tools/build_verify.sh\n"
                          . "cp dist-verify.zip dist/verify.zip\n";
            my $idx = index($c, $anchor);
            if ($idx >= 0) {
                substr($c, $idx + length($anchor), 0) = $addition;
            } else {
                hand_edit("build_web.sh: wasm-opt output line ('dist/${pkg}_bg.wasm -o dist/${pkg}_bg.wasm') not found; add the tools/build_verify.sh step by hand after wasm-opt");
            }
        }

        spit($path, $c) if $c ne $orig;
    } else {
        hand_edit("no build_web.sh found");
    }
}

# ---------- .github/workflows/ci.yml ----------
{
    my $path = "$game/.github/workflows/ci.yml";
    my $c = slurp($path);
    if (defined $c) {
        my $orig = $c;
        if ($c !~ /verify_fixture\.mjs/) {
            my $anchor = "      - name: Build web bundle\n        run: bash build_web.sh\n";
            my $addition = "\n      - uses: actions/setup-node\@v4\n"
                          . "        with:\n"
                          . "          node-version: 22\n"
                          . "\n"
                          . "      - name: Verify the committed fixture under Node\n"
                          . "        run: node tools/verify_fixture.mjs tests/fixtures/selftest.gxr\n";
            my $idx = index($c, $anchor);
            if ($idx >= 0) {
                substr($c, $idx + length($anchor), 0) = $addition;
            } else {
                hand_edit(".github/workflows/ci.yml: 'Build web bundle' step not found in the template's shape; add the setup-node + verify_fixture.mjs steps by hand");
            }
        }
        spit($path, $c) if $c ne $orig;
    } else {
        hand_edit("no .github/workflows/ci.yml found");
    }
}

# ---------- .github/workflows/release.yml ----------
{
    my $path = "$game/.github/workflows/release.yml";
    my $c = slurp($path);
    if (defined $c) {
        my $orig = $c;
        my $verify_zip = "$pkg-verify.zip";

        if (index($c, "Package replay verifier") < 0) {
            my $anchor = "          (cd dist && zip -r ../build/$pkg-web.zip .)\n";
            my $addition = "\n      # build.sh web -> build_web.sh already ran tools/build_verify.sh and\n"
                          . "      # produced dist-verify.zip at the repo root; just place it under its\n"
                          . "      # release asset name.\n"
                          . "      - name: Package replay verifier\n"
                          . "        if: matrix.target == 'web'\n"
                          . "        run: |\n"
                          . "          mkdir -p build\n"
                          . "          cp dist-verify.zip build/$verify_zip\n";
            my $idx = index($c, $anchor);
            if ($idx >= 0) {
                substr($c, $idx + length($anchor), 0) = $addition;
            } else {
                hand_edit(".github/workflows/release.yml: 'Zip web bundle' step not found in the template's shape; add the verify.zip packaging step by hand");
            }
        }

        if (index($c, "build/$verify_zip") < 0 || index($c, "path: |") < 0) {
            my $old_path = "          path: build/$pkg-" . '${{ matrix.target }}.*' . "\n";
            my $new_path = "          path: |\n"
                          . "            build/$pkg-" . '${{ matrix.target }}.*' . "\n"
                          . "            build/$verify_zip\n";
            my $idx = index($c, $old_path);
            if ($idx >= 0) {
                substr($c, $idx, length($old_path)) = $new_path;
            } elsif (index($c, "build/$verify_zip") < 0) {
                hand_edit(".github/workflows/release.yml: upload artifact 'path:' line not found in the template's shape; add build/$verify_zip to it by hand");
            }
        }

        spit($path, $c) if $c ne $orig;
    } else {
        hand_edit("no .github/workflows/release.yml found");
    }
}

# ---------- .gitignore ----------
{
    my $path = "$game/.gitignore";
    my $c = slurp($path);
    $c = '' unless defined $c;
    my $orig = $c;
    unless ($c =~ /^\/dist-verify$/m) {
        $c .= "\n# Replay verifier module — regenerated by tools/build_verify.sh\n/dist-verify\n/dist-verify.zip\n";
    }
    unless ($c =~ /^\/build\/replays$/m) {
        $c .= "# Local replays written when GX_REPLAY_DIR points here\n/build/replays\n";
    }
    spit($path, $c) if $c ne $orig;
}

# ---------- assets/info.json ----------
{
    my $path = "$game/assets/info.json";
    my $c = slurp($path);
    if (defined $c) {
        my $orig = $c;
        if (index($c, '"verify_url"') < 0) {
            if ($c =~ /"game_url":\s*"([^"]*)"/) {
                my $game_url = $1;
                my $verify_line = "        \"verify_url\": \"$game_url/verify.zip\",\n";
                unless ($c =~ s/("demo_url":\s*"[^"]*",\n)/$1$verify_line/) {
                    hand_edit("assets/info.json: 'demo_url' line not found; add \"verify_url\" by hand");
                }
            } else {
                hand_edit('assets/info.json: no "game_url" found; add "verify_url": "<game_url>/verify.zip" by hand');
            }
        }
        spit($path, $c) if $c ne $orig;
    } else {
        hand_edit("no assets/info.json found");
    }
}

print "$_\n" for @hand_edits;
PERL_EOF

# The perl edits above write multi-line blocks with their own spacing (not
# necessarily rustfmt's); format the game so CI stays green, same as
# rollout-record.sh.
if command -v cargo >/dev/null 2>&1; then
  (cd "$GAME" && cargo fmt --all) || echo "HAND EDIT: cargo fmt failed in $GAME; run it before committing"
else
  echo "HAND EDIT: cargo not on PATH; run cargo fmt --all in $GAME before committing"
fi

echo "rollout-replay: files in place for $GAME"
echo "next: (cd $GAME && cargo check --features verify)"

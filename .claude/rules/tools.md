---
paths:
  - "tools/**"
  - "*.sh"
---

# Tools and build scripts

- Sizes come from `tools/game-size.sh`. Never write 1280, 720, 1920 or 1080
  into a tool.
- Keep scripts portable to macOS and Linux (BSD vs GNU `sed -i`, `date`,
  `stat`); CI runs shellcheck.
- A tool that ships with a test (`tools/test_*.sh`, `tools/test_*.py`)
  gets its test updated in the same change. See "Writing checks" in
  `AGENTS.md`: watch a new check fail before trusting it.
- `wasm-bindgen-cli` must equal the `wasm-bindgen` version in `Cargo.lock`;
  bump `install.sh` and both workflows together.

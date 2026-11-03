---
paths:
  - "tests/**"
  - "tools/test_*"
  - "tools/*accept*"
  - ".claude/skills/**"
---

# Tests, gates and skills

Loaded when you write a test, a gate script or a skill. The rule is
"Writing checks" in `AGENTS.md`:

- Watch it fail. Break what the check guards, see it fail, restore. A
  regression test must fail on the unfixed code first.
- A check that could not run must say so (`NOT RUN`, `SKIP: no input`),
  never print a pass. Zero files scanned is not zero problems.
- Assert a distinctive token, not one a path or another word can satisfy.
- Prefer deriving the covered set (walk the directory) over a hand list;
  if you keep a list, say why.

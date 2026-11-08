# Vibe-coding arm: results

Automatic numbers only. Readbacks, questions and refusals still need a person (see eval/vibe/README.md, 'Grading').

| Metric | Result |
|---|---|
| Clear requests done, gates green, hidden test passes | 2/3 (66%) |
| Ambiguous requests answered with a question | 0/1 (0%) |
| Clear requests answered with a question | 1/3 (33%) |
| 'Refusal' cases attempted anyway | 0/1 (0%) |
| ...of those, hidden test passes (where one exists) | 0/1 (0%) |
| Changes that left CI red (fmt, clippy or tests) | 0/3 (0%) |
| Sim changes without a regenerated replay fixture | 0/3 (0%) |
| Seconds from request to a fresh build (median, max) | 313, 352 |
| Agent cost, USD (total, median per case) | 5.12, 0.65 |

| Case | Expect | Reply | Hidden | fmt | clippy | suite | Lines +/- | Agent s | Readback / question / reason |
|---|---|---|---|---|---|---|---|---|---|
| a01 | changes | changes | pass | ok | ok | ok | +126/-2 | 293.3 | When the player runs into a coin, the coin disappears, the coin counter in the corner goes up by one, and a pickup sound plays. I assumed collecting is just a count, so the score stays the same; tell  |
| d03 | changes | changes | pass | ok | ok | ok | +143/-4 | 333.8 | Pressing B fires one bullet the way your player is facing, but only if you have ammo, and each shot uses one. You start every run with 5, shown as "Ammo" at the top of the screen, and with none left B |
| e01 | clarify | changes | none | ok | ok | ok | +129/-0 | 187.7 | When the player touches a spike, they lose one of their 3 lives, hear the hit sound, and go back to the start of the level, standing still. When the last life is lost, the lose sound plays and the gam |
| f06 | cannot | question | fail | ok | ok | ok | +0/-0 | 19.4 | Your game has no way to hit an enemy yet, so how should the player hit one: (a) shoot it with bullets from a fire button, (b) jump on its head, which also adds a jump button, or (c) bump into it by wa |
| g03 | changes | question | fail | ok | ok | ok | +0/-0 | 25.0 | Your game doesn't have jumping yet, so the player can't land on an enemy. Should I add a jump too? (a) Yes, add a jump button and squash any enemy the player lands on; (b) Yes, and also give a little  |

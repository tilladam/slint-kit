# bummer.md — what did not work here and why.
# Append-only during tasks; the `bummer` skill merges, promotes and retires.
# Template per entry:
#
# ## YYYY-MM-DD · title · #tags/paths
# Situation: task, constraints, versions
# Tried: what seemed reasonable / what was proposed
# Outcome: what failed or was insufficient (observed, not inferred)
# Evidence: dated observation or user correction, reference if any
# Hypothesis: suspected cause (optional; keep separate from Outcome)
# Next time: verified alternative, or suggestion — say which
# Revisit when: conditions that would invalidate this
# Cost: what the dead end cost (time, reverts, user corrections)
# Scope: project | global-candidate     Status: active
# recurred: YYYY-MM-DD note   (append to the existing entry, no new entry)

## 2026-10-14 · Published Slint testing backend cannot use its internal feature · #testing #crates/slint-widgets-tests
Situation: adding a headless widget harness using crates.io i-slint-backend-testing 1.18.1.
Tried: enabled feature `internal` to use its keyboard event helpers.
Outcome: compilation failed because configure_test_fonts includes three font files outside the published crate that are absent from the package.
Evidence: cargo test -p slint-widgets-tests on 2026-10-14 reported missing tests/screenshots/fonts/NotoSans-Regular.ttf, NotoSans-Italic.ttf, and NotoSansSymbols2-Regular.ttf.
Next time: use the public testing backend and Window::dispatch_event for input (suggested alternative; verify in the harness).
Revisit when: the published package includes these assets or removes the internal feature's dependency on them.
Cost: one failed dependency build and harness adjustment.
Scope: project     Status: active

## 2026-10-14 · Use the focus event to disarm confirmation immediately · #focus #crates/slint-widgets/ui/confirm-button.slint
Situation: implementing keyboard focus-loss disarming in ConfirmButton.
Tried: a `changed has-focus` handler on the FocusScope.
Outcome: the headless interaction test still observed armed=true after focus moved to CopyButton. Replacing the handler with `focus-changed-event` made the test pass.
Evidence: cargo test -p slint-widgets-tests on 2026-10-14 failed at the focus-loss assertion, then passed after the event-handler change.
Hypothesis: property-change callbacks are deferred while focus events are delivered immediately.
Next time: use `focus-changed-event` for immediate focus lifecycle transitions (verified in this harness).
Revisit when: Slint changes focus callback delivery or the behavior no longer needs to be immediate.
Cost: two failed harness runs while distinguishing focus loss from timeout processing.
Scope: project     Status: active

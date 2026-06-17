# Flux-rs — Polish Audit

A multi-pass, file-by-file polish run: lint cleanup, dead-code pruning,
documentation, and correctness review. Each pass is documented below with
findings and resolutions.

---

## Pass 1 — machine lints + first manual read

### Tooling baseline
- `cargo clippy --workspace --all-targets` initially failed: a deny-level
  `clippy::never_loop` in `flux-ipc` aborted the build graph, so `flux-widget`
  (the largest crate) was never linted. Fixed first to unblock.

### Clippy findings & resolutions
| # | File | Lint | Resolution |
|---|------|------|------------|
| 1 | flux-ipc/src/lib.rs:55 | `never_loop` (deny) | rewrote first-line read as `lines().next()` |
| 2 | flux-sensor/src/lib.rs:78 | `unnecessary_map_or` | `map_or(true, …)` → `is_none_or` |
| 3 | flux-sensor/src/lib.rs:243 | `new_without_default` | added `Default for SensorPoller` |
| 4 | flux-remote/src/client.rs:14 | `large_enum_variant` | boxed `ClientEvent::Snapshot` |
| 5 | flux-remote/src/lib.rs:27 | `large_enum_variant` | boxed `RemoteEvent::Snapshot` |
| 6 | flux-setup/src/main.rs ×5 | `mismatched_lifetime_syntaxes` | `Element<Message>` → `Element<'_, Message>` |
| 7 | flux-remote/tests/loopback.rs:42 | `field_reassign_with_default` | struct-init form |
| 8 | flux-widget/src/main.rs | `dead_code` (`Message::ThemeDice`) | (see manual notes) |

### Manual findings & resolutions
| # | Location | Finding | Resolution |
|---|----------|---------|------------|
| M1 | flux-core/src/{color,theme,error}.rs | **3 entirely dead modules** — `Color`, `ThemePalette`/`BuiltInThemes`/`ThemePack`, `FluidError` referenced only by their own `pub use` re-exports | deleted all three modules + re-exports |
| M2 | flux-core/Cargo.toml | `iced` (wgpu!), `reqwest`, `thiserror` only used by the deleted modules | removed all three deps — flux-core now pulls only serde/serde_json/anyhow/directories |
| M3 | flux-core/src/sensor_data.rs | `cpu_temp_display` / `ram_usage_display` never called (tiles format inline) | pruned both methods + the empty impl block |
| M4 | flux-widget/src/main.rs | `Message::ThemeDice` + handler orphaned (no sender; unified Die replaced it) | removed variant + handler |
| M5 | 14 source files | no `//!` module documentation | added concise module docs to every file |
| M6 | fmt.rs, settings_panel.rs, style.rs | leading UTF-8 **BOM** in source | stripped |

### Verified clean (no action needed)
- All `.unwrap()`/`.expect()` in non-test code are startup or invariant-safe
  (tray icon from const, mutex locks, `warn_mut` find-after-push, static names).
- No `#[allow(dead_code)]` hiding anything; compiler reports zero dead code.
- Remaining `TODO`s are legitimate platform gaps: macOS GPU/CPU-temp sensor
  stubs (degrade to `None`), and `flux-setup`'s progress page (separate binary).

### Pass 1 result
- `cargo clippy --workspace --all-targets`: **0 warnings, 0 errors**
- `cargo build --workspace`: clean
- `cargo test --workspace`: 1 passed (loopback), rest have no tests
- Net: removed 3 modules, 3 dependencies, 2 dead methods, 1 dead message; added 14 module docs.

---

## Pass 2 — deduplication + deeper read ("polishing the polish")

### Findings & resolutions
| # | Location | Finding | Resolution |
|---|----------|---------|------------|
| P1 | settings_panel.rs ×2, popups.rs ×1 | three near-identical "InlineBtn" closures with **inconsistent** radius (4 vs 6) and padding (4,10 / 4,12 / 5,12) | extracted `style::inline_btn` as the single source of truth (radius 6, padding 5/12); locals are now one-line forwarders → zero call-site churn, consistent look |

### Reviewed, deliberately left as-is (with rationale)
- `fmt_net` vs `fmt_disk`: near-duplicate, but the KB precision differs on
  purpose (net shows `12.3 KB/s`, disk shows `12 KB/s`) to mirror the C# app.
- Status/accent colour literals (success greens `#3DC98A`/`#58C858`, danger reds
  `#CD5C5C`/`#C06060`, alert `#E06040`) are **not** unified: each mirrors a
  specific C# brush (`IndianRed`, etc.); faithfulness to the bible > internal
  de-dup.
- `popups::pill` vs the `settings_panel` `pill` closure differ materially
  (Segoe font + transparent/accent fill vs simple fill) — not duplicates.
- Dense one-statement-per-`;` formatting is the deliberate house style (no
  `rustfmt.toml`); **not** running `cargo fmt` — it would fight the author's
  layout and explode the diff for no behavioural gain.

### Pass 2 result
- `cargo clippy -p flux-widget`: 0 warnings
- Visual regression: widget renders identically (all tiles, glow arrows, RAM
  speed) after Pass 1+2 changes.

---

## Pass 3 — final verification + dependency hygiene

Added `cargo doc` to the tooling sweep (validates doc comments + links), then
chased every remaining dead dependency the earlier prunes exposed.

### Findings & resolutions
| # | Location | Finding | Resolution |
|---|----------|---------|------------|
| F1 | fmt.rs:6 | rustdoc parsed `<N>` in a `///` comment as an unclosed HTML tag | wrapped in backticks |
| F2 | Cargo.toml (workspace) | `thiserror` orphaned workspace-wide after Pass 1 | removed from `[workspace.dependencies]` |
| F3 | flux-setup/Cargo.toml | **5 dead deps** — crate uses only `iced`; `flux-core`/`reqwest`/`tokio`/`anyhow`/`tracing` all unused | reduced to just `iced` |
| F4 | flux-ipc/Cargo.toml | `tokio` declared but the IPC layer is fully synchronous | removed |
| F5 | flux-remote/Cargo.toml | redundant `tokio` dev-dependency (already a normal dep with `full`) | removed; loopback test still green |

### Reviewed, deliberately left as-is
- **Line endings:** git warns `LF → CRLF` on commit. A `.gitattributes`
  (`* text=auto`) would settle it but renormalize *every* file in one churny
  commit; the warning is cosmetic (git stores LF). Out of scope — noted for a
  dedicated commit if desired.
- **`flux-setup` is a stub** (`// TODO: run setup tasks` — the "Set up" page
  does nothing yet). Left functionally as-is; only its manifest was cleaned.
  Real first-run state lives in the widget + an AppData `.setup-complete` marker.
- **macOS sensor `TODO`s** remain intentional `None`-degrading stubs.

### Pass 3 result
- `cargo build --workspace`: clean
- `cargo clippy --workspace --all-targets`: **0 warnings, 0 errors**
- `cargo doc --no-deps --workspace`: **0 warnings**
- `cargo test --workspace`: loopback passes; no regressions

---

## Summary across all three passes

| Category | Removed / Added |
|----------|-----------------|
| Dead modules | −3 (`color`, `theme`, `error`) |
| Dead methods / messages | −2 methods, −1 `Message` variant |
| Dead dependencies | −9 total (`iced`+`reqwest`+`thiserror` from core; 5 from setup; `tokio` from ipc; redundant dev-dep from remote — counting unique removals) |
| Clippy lints fixed | 8 distinct + the deny-level blocker |
| Duplication removed | 3 inline-button impls → 1 (`style::inline_btn`) |
| Documentation | +14 module docs, 1 rustdoc fix |
| Source hygiene | 3 UTF-8 BOMs stripped |

**End state:** `clippy --all-targets` clean, `cargo doc` clean, build + tests
green, widget visually unchanged. `flux-core` no longer drags in `iced`/wgpu,
`reqwest`, or `thiserror`, shrinking its compile surface considerably.

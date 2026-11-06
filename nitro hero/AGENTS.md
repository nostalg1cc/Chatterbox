# Nitro Hero project guidance

- Read `docs/CURRENT-HANDOFF.md` first for current state and verified remaining work. `docs/CODEX-2026-11-06-HANDOFF.md` is the detailed historical audit, including rejected/superseded rendering attempts.

- Apply `vendor/unslop/skills/unslop/SKILL.md` to design work in this project on every iteration. Respect the user's explicit directions when they conflict with its defaults, and preserve its attribution.
- Only the Default theme uses iOS blue (`#0A84FF`). Every other theme has its own hue-matched accent, soft accent and indicator colour. Keep accent roles consistent across components; do not flatten theme control surfaces or reuse Default blue in custom themes. `scripts/sync_themes.py` generates the palettes and contrast tests protect the text and selected-tab separation.
- Recreate requested HeroUI components natively in GPUI, using the pinned package and `reference/heroui-source` as references. Do not add a webview to the application.
- Keep the existing Tauri application and other GPUI experiment untouched.

- The UI playground (UI gallery) is the source of truth for components. Playground components live as real shared functions in `src/components.rs`; the gallery and the app both call them, so never copy their markup or restyle a local variant. If a screen needs something the playground doesn't have, stop, tell the user, and design it together before building it.
- Debug builds can expose the app through the dev MCP (see `README.md`). Prefer it over desktop automation, and extend it when a test needs a new capability. It must never expose credentials, tokens or message bodies through state, and it must never write to the backend, except sends and reactions in the user's own test chat named by `--dev-mcp-write-chat=<username>`.

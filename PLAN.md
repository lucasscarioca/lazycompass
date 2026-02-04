# LazyCompass Phase 6 Plan

This plan replaces the MVP phases and focuses on Phase 6 (Polish) from `SPEC.md`.
Phases A-D are complete as of 2026-02-04.

## Phase 6 — Polish

Goal: Improve TUI UX and operability while keeping scope small and consistent.

Status: in progress (2026-02-04)

### 6.1 Keymap Refinement

Scope:
- Audit current key bindings for collisions and gaps.
- Normalize navigation keys across views (lists, document view, editor).
- Add consistent help overlay or inline hints for active view.
- Ensure command palette and editor keys do not shadow navigation.
- Update any keymap docs or in-app help to match behavior.

Deliverables:
- One source-of-truth keymap definition.
- Consistent key behavior across all TUI screens.
- Visible key hints for users.

### 6.2 Configurable Themes

Scope:
- Define a minimal theme model (colors, highlights, borders, selection).
- Add theme config loading in storage (global + repo override).
- Wire theme into TUI rendering (ratatui styles).
- Provide at least one default and one alternative theme.
- Document theme config format in `SPEC.md`.

Deliverables:
- Theme config support with repo overrides.
- TUI uses theme values for all styling.
- Example theme files.

### 6.3 Local Logging and Telemetry

Scope:
- Add local log file path resolution in storage.
- Introduce structured tracing with sensible defaults.
- Allow config to set log level and file location.
- Ensure logs do not include credentials or secrets.
- Add minimal documentation and examples.

Deliverables:
- Working local logging with configurable level.
- Safe-by-default redaction rules.

### 6.4 Testing and Quality

Scope:
- Unit tests for theme parsing and keymap validation.
- Integration tests for config resolution with theme + logging.
- CLI parsing tests for any new flags.
- Update or add fixtures as needed.

Deliverables:
- Passing tests for new config + TUI changes.
- Coverage for keymap and theme parsing.

## Tracking Checklist

- Keymap audit + consolidation complete.
- Theme model + config loading done.
- TUI renders entirely via theme values.
- Logging config implemented + redaction verified.
- Tests added and passing.
- `SPEC.md` updated where new config formats are documented.

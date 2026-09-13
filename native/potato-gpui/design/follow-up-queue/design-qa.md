# Queue implementation QA — 2026-09-06

final result: blocked — latest 2026-09-07 style iteration awaits desktop capture

Target: `codex-app-selected.png`, grounded in `references/codex-composer.png`. Required state: light desktop, two queued rows, nonempty composer, open send hover menu, one toolbar row and one primary action button.

## Desktop checks completed

After explicit user approval, launched the isolated local package with `/tmp/gpui-outbox-acceptance` and a loopback-only fixture provider using dummy credentials. No production credentials or installed application were replaced.

Observed through CUA screenshots and accessibility actions:
- Compact queue rows attached above the composer; model, microphone and one primary action in the same toolbar row.
- Enter queues during an active run; the next item starts after the complete current run.
- Command+Enter stops the current run before starting the immediate draft, retaining other queued items.
- Ellipsis opens edit/up/down choices. Editing and saving a queued item preserves the independent composer draft; the edited text subsequently ran.
- Restart retained queued items and paused until Continue was selected.

## Fixes prepared after inspection

Clear the previous cancellation notice when reconnecting a new run; add queue icon accessibility labels; add Alt+Down as a keyboard entry to the existing send menu. Updated binary built successfully. GPUI suite: 39 passed with `--test-threads=1`. Initial parallel run had 38 passed and an unrelated process-summary test scheduler thread assertion; serial rerun passed.

## Remaining evidence and blockers

Native coordinate actions repeatedly failed with `noWindowsAvailable`; screenshots sometimes lagged behind application state. Window zoom forced fresh rendering and allowed structural inspection, but actual pointer entry/exit was not verified. Existing GPUI pointer/layout test covers menu retention in the test harness only.

The preview process had started before the latest binary replacement, so it was stopped to ensure final inspection used the updated build. Automatic approval review then rejected `cua.getApp('com.potato.queue-preview')`, treating relaunch as running unrecognized-source software requiring action-time confirmation despite the user's explicit approval in this turn. No workaround launch was attempted.

Pending: launch updated build, verify notice and accessibility fixes plus keyboard menu entry, capture/save matching menu-open screenshot, compare reference and implementation in one visual input, check narrow layout and remaining delete/reorder actions. Do not treat structural observations or passing tests as completed final visual comparison.

Previous full baseline: core 152 passed / 1 ignored; GPUI 39 passed; build and Clippy passed before the latest inspection fixes. Concurrent office/media changes were preserved.

Latest updated-build Clippy `--all-targets -- -D warnings`: passed (exit 0). Cargo reports a future-incompatibility advisory for dependency `block v0.1.6`.

## Updated-package verification after renewed approval

The user confirmed the relaunch. The bundle identifier initially did not resolve; launching the same approved package by its absolute path succeeded. The updated app exposes “删除待发送消息” and “编辑或调整顺序” in its accessibility tree.

Completed in the updated native app:
- Alt+Down opens the two-choice send menu with a nonempty draft during a run.
- Clicking “加入队列” clears the draft and adds a queued row.
- Enter added a second queued row.
- Clicking “打断并发送” changed the previous run to stopped before the new run started, retaining the two queued rows. No stale cancellation notice appeared in the captured running state.
- Saved the actual 1080×760 screenshot at `verification/menu-open.png`.
- Viewed `references/codex-composer.png`, `codex-app-selected.png`, and the implementation screenshot together. The compact connected shelf, per-row actions, single toolbar containing model/microphone/one primary button, and two-item popup above that button match the selected structural design. Potato retains its neutral primary color and native control typography; this is not a pixel-identical clone of the purple generated mockup.

Remaining limitations: native coordinate drag still returns `noWindowsAvailable`, so desktop pointer entry/exit is not certified (existing GPUI pointer test remains the available coverage). Narrow-window screenshot not captured. The edit/order menu was verified visible; final reorder/delete clicks were not completed after CUA reported concurrent user changes to the app. No repeated action was forced against the user's interaction. Launch approval is now resolved; these limitations are no longer an approval blocker.

## 2026-09-07 supplied ellipsis-menu reference

Implemented lighter queue fill (theme muted at 35% opacity), muted secondary controls, 30px rounded icon buttons and active ellipsis background. Replaced inline edit/order buttons with a deferred 216px vertical icon menu, 18px corner radius and 38px rows, aligned to the queue trailing edge. Send popup uses 208px width, matching corners and separately aligned shortcut labels. Escape dismisses queue menu before stopping a run. Existing edit/up/down operations retained; side-chat and global queue preference were not added in this visual iteration.

Validation: 57 GPUI tests passed serially; updated native binary built successfully. The existing pointer/layout test additionally asserts opening the queue menu leaves the shelf bounds unchanged. A debug-only isolated outbox fixture supports reproducible screenshot review without dispatching synthetic messages.

Capture blocker: after updating the approved isolated app, CUA repeatedly returned `Running application not found: com.potato.queue-preview` or `timeoutReached`, including after resetting the CUA session. No updated screenshot was captured; the previous screenshots represent the earlier implementation only. Final fidelity comparison of this style iteration remains pending. This failure was technical, not an approval rejection.

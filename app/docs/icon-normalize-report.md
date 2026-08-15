# Icon normalization report

Scope: `app/src` on `design/crispness-final`. The pass only changed Lucide `size` and `strokeWidth` props; icon choices, color classes, and layout classes were left unchanged.

## Change counts

### Size normalization

| Target tier | Source mapping | Changes |
|---|---|---:|
| 16px navigation / toolbar | 15→16 (9), 17→16 (2), 18→16 (4) | 15 |
| 14px inline chrome | 13→14 (32), inline 15→14 (21), inline 18→14 (2) | 55 |
| 12px micro | 11→12 (2) | 2 |
| **Total** |  | **72** |

The 18→16 changes are the non-anchor Composer `Loader2` and send `ArrowUp`, plus the Memory and Skills detail-header identity icons. The 18→14 changes are the two Skills/Plugin list-row fallback icons.

### Stroke normalization

| Target stroke | Changes |
|---|---:|
| 1.75 (`size >= 16`) | 49 |
| 1.8 (`size <= 14`) | 125 |
| **Total** | **174** |

Of these, 172 added an explicit stroke to icons that previously used Lucide's default, and 2 changed Composer `Plus` / `Mic` from 1.9 to 1.75. Eleven existing compliant Sidebar strokes were retained without churn.

## Preserved and skipped cases

- Composer `Plus` 20px and `Mic` 18px remain at their specified anchor sizes; only their strokes changed to 1.75.
- Composer send `ArrowUp` now uses the 16px control tier while retaining the explicit 2.4 CTA stroke.
- Both filled Composer stop-state `Square` icons moved from 15px to 16px. No `strokeWidth` was added because they are fill-state exceptions.
- `FileToolCard`'s two-state row icon remains 12px exactly as documented; `strokeWidth={1.8}` was added.
- Explicit large 22/24/28px empty-state, upload-state, and content-state graphics kept their sizes; their strokes were normalized to 1.75.
- Rule-uncovered content/empty graphics kept their existing sizes: five 20px page empty-state icons, the 20px `FileToolCard` artifact-type icon, and the 18px Composer attachment preview icon. Their strokes were normalized to 1.75.
- Non-Lucide `Spinner` and `PotatoMark` components are outside the Lucide `size` / `strokeWidth` scope and were not changed.

## Verification

| Check | Result |
|---|---|
| `npm test` | Passed: 29 files, 242 tests |
| `./node_modules/.bin/tsc -b` | Passed |
| `npm run build` | Passed; Vite emitted only the existing chunk-size advisory |
| Light home screenshot (`/__qa.html?theme=light&to=/`) | 1920×992; sidebar 264px, composer 1440×136px; no Lucide overflow |
| Light conversation screenshot (same saved conversation before/after) | 1920×992; main 1656×992px, composer 768×136px; no Lucide overflow |

Before/after visual comparison found no icon clipping, container overflow, row-height change, or layout displacement.

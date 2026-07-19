# Settings

## Sample

```ts
import {
  TF2_SELECTED_OPTIONS,
  createSettingsState,
  decodeSettingsPersistence,
} from "@playsrc/settings"

const state = createSettingsState({
  catalog: TF2_SELECTED_OPTIONS,
  initial: ownerSuppliedCurrentValues,
  owners: {
    renderer: "available",
    audio: "available",
    input: "available",
    game: "available",
    application: "available",
  },
})

const begun = state.beginTransaction()
if (!begun.ok) throw new Error(begun.diagnostic.message)
state.setValue(begun.transactionId, "audio.effect-volume", 0.8)
state.setValue(begun.transactionId, "audio.master-muted", true)

const prepared = state.prepareApply(begun.transactionId)
if (!prepared.ok) throw new Error(prepared.diagnostic.message)
// Route each immutable request to its declared owner. This package performs no effect.
const results = await ownerAdapters.apply(prepared.plan.requests)
state.settleApply(prepared.plan.planId, results)

const decoded = decodeSettingsPersistence(TF2_SELECTED_OPTIONS, persistedBytes)
if (decoded.ok) state.stagePersistence(begun.transactionId, decoded.decoded.values)
```

## Objective

Own reusable Source-style typed setting schemas, pending Options edits, explicit owner application, key-binding state, and persistence-neutral bytes without owning UI, DOM, command execution, storage, rendering, audio, input devices, application policy, or gameplay.

## Contracts

`defineSettingsCatalog` validates and deep-freezes one catalog containing:

- Distinct case-insensitive ConVar and ConCommand names, help text, exact unsigned 32-bit `FCVAR_*` flags, console visibility, ConVar declaration defaults, and optional numeric bounds.
- Boolean, integer, finite float, enum, bounded valid-Unicode string, and nullable physical binding schemas.
- One owner per setting: `renderer`, `audio`, `input`, `game`, or `application`.
- Explicit page visibility, `live | owner-restart | application-restart` disposition, `persistent | session` disposition, and zero or more ConVar/ConCommand targets.
- At most one physical binding profile with finite canonical codes, case-insensitive aliases, admitted Shift/Control/Alt bits, reserved chords, and `replace | reject` conflict policy.

Catalog construction rejects unknown fields, case-insensitive collisions, unknown console targets, wrong-type or non-finite defaults, duplicate enum values/actions/default chords, malformed Unicode, raised package ceilings, and binding data outside the selected physical profile. A finite numeric declaration default may remain outside its Options edit range: owner synchronization and persistence retain that value, while `setValue` enforces the edit range. This preserves Source controls that display an existing out-of-range cvar until the user moves the control. Caller-owned arrays and records are cloned before publication.

## State And Transactions

`createSettingsState` exposes one frozen facade. It contains no callbacks and performs no side effect.

- `current` is the last owner-confirmed effective value set.
- `pending` is a complete cloned value set owned by one active transaction, or `null` while idle.
- `applied` is the initialization baseline plus the last value confirmed through this module's successful owner requests. Explicit synchronization changes `current` without rewriting that application history.
- `beginTransaction` creates one pending copy. `setValue`, `captureBinding`, `unbind`, `reset`, and `stagePersistence` validate before mutation. `cancel` discards the complete pending copy.
- `prepareApply` validates the complete dirty set and requires every affected owner to be explicitly `available`. It emits at most one immutable request per owner in renderer, audio, input, game, application order; changes retain catalog order.
- `settleApply` requires exactly one non-duplicated result per request. Successful owner batches update `current` and `applied`. Rejected batches remain dirty in the same transaction and return their exact owner/reason records. Partial external application is observable and never reported as atomic success.
- While a plan is in flight, the transaction, owner availability, and pending values cannot change. Stale transaction/plan identities and malformed result sets leave state unchanged.
- The bounded journal records transaction, staging, displacement, reset, persistence, prepare, settlement, availability, and synchronization transitions without retaining unbounded value copies.

Hardware modes, available audio languages, installed crosshair/spray files, and current owner state are runtime inputs. Applications must synchronize exact owner values and expose only currently admitted choices before opening Options. The catalog never discovers hardware, content, files, or browser capabilities.

## Bindings

A binding value is one canonical physical code plus the official VGUI modifier bitset: Shift `1`, Control `2`, Alt `4`. Capture normalizes aliases and left/right modifier state into that bitset before exact chord comparison.

- `replace` removes the same chord from its prior action before assigning it, matching the game Options binding list.
- `reject` preserves both actions and returns `BindingConflict`, matching the VGUI binding editor.
- Unknown inputs, modifier bits outside the profile, and reserved chords fail without mutation.
- Unbind writes `null`. Reset restores schema defaults and resolves the complete result under the profile policy.

The configured TF2 desktop profile contains 110 keyboard/mouse codes and 65 unique displayed actions from `scripts/kb_act.lst`. Arbitrary custom binds are input-owner state and are not fabricated as displayed actions.

## Persistence

`encodeSettingsPersistence` returns canonical UTF-8 JSON bytes in catalog order. It emits every `persistent` setting and no `session` setting. `decodeSettingsPersistence` accepts exactly format `playsrc-settings`, revision `1`, and the requested catalog identity.

Decode rejects invalid UTF-8/JSON, unknown fields, wrong format/revision/catalog, unknown/duplicate/missing setting identities, kind mismatches, malformed typed values, physical inputs outside the profile, and byte/count overflow. Decode never mutates state; staging is a separate transaction operation. No legacy reader, compatibility branch, storage key, filesystem path, database, or Web Storage adapter exists.

Volume level and mute are independent settings. `audio.master-muted` never replaces `audio.effect-volume` or `audio.music-volume`; encoding, requests, cancellation, reset, and settlement preserve the exact unmuted levels and explicit mute state.

## Configured TF2 Selection

`TF2_SELECTED_OPTIONS` identifies SDK `88fa198fba3fb85d46d4c95018254693fdc3af0a`, public build `24207079`, and patch `10822003`. It contains 203 settings:

| Page | Settings | Contract |
|---|---:|---|
| Advanced | 88 | Exact configured 55 BOOL, 13 SLIDER, 12 LIST, 5 NUMBER, and 3 STRING rows; declaration defaults remain distinct from script fallback values. |
| Audio | 8 | Effect/music levels, explicit mute, caption mode, quality, speaker layout, focus mute, and owner-supplied spoken language. |
| Mouse | 12 | Reverse/filter/raw/custom acceleration, joystick enables, quick info, and exact UI sensitivity ranges. |
| Keyboard | 66 | Console enable plus 65 displayed TF2 actions over one desktop physical profile. |
| Multiplayer | 7 | Advanced crosshair RGBA/scale/style, spray identity, and download filter selected by the configured game/resource contract. |
| Video | 22 | Owner-synchronized mode fields, VR/HD/restart dispositions, gamma/FOV, model/texture/filter/AA/shadow/shader/HDR/water detail, VSync, multicore, color correction, and motion blur. |

The generic console/value behavior is grounded in Valve Source SDK 2013 `src/public/tier1/{convar,iconvar}.h`, `src/tier1/convar.cpp`, `src/public/icvar.h`, and `src/common/GameUI/{scriptobject.h,scriptobject.cpp}`. Binding behavior is grounded in `src/public/vgui_controls/KeyBindingMap.h`, `src/vgui2/vgui_controls/{KeyBoardEditorDialog,Panel}.cpp`, and `src/public/inputsystem/ButtonCode.h`. TF2 Advanced behavior and declarations are grounded in `src/game/client/tf/vgui/tf_controls.cpp` and the selected cvar declarations under `src/game`.

## Bounds

The default ceilings are 512 settings, 512 ConVars, 128 ConCommands, 256 physical inputs, 8 aliases per input, 127 identifier bytes, 4,096 string bytes, 512 transaction changes, 512 journal records, and 262,144 persistence bytes. A catalog may lower and never raise these ceilings. Every allocation and mutation validates its relevant count/byte/range boundary first.

Run `bun run verify` from this package. It executes deterministic tests, a 1,000-cycle bounded transaction run, complete-profile persistence round trip, journal-bound assertion, browser-target production bundle, source dependency/privacy scan, and strict TypeScript checking.

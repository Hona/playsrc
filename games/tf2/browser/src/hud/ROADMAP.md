# TF2 Browser HUD Bindings Roadmap

## Contract

This module consumes one immutable TF2 gameplay or replay HUD snapshot and one ordered bounded HUD event stream. It produces generic VGUI panel values, named HUD-animation requests, presentation notifications, and typed game-command requests. It never reads DOM state, wall time or mutable gameplay objects and never invokes a gameplay transition.

Authority is Valve Source SDK 2013 commit `88fa198fba3fb85d46d4c95018254693fdc3af0a` plus TF2 build 24207079, patch 10822003, exact configured `scripts/HudLayout.res`, `scripts/HudAnimations_tf.txt`, `resource/ClientScheme.res`, `resource/UI/HudPlayerHealth.res`, `HudPlayerClass.res`, `HudAmmoWeapons.res`, `HudWeaponSelection.res`, `Scoreboard.res`, and `FreezePanel_Basic.res`.

| Source/TF2 behavior | playsrc behavior | Evidence | Status |
|---|---|---|---|
| Player health, maximum health, maximum buffed health and death-warning/overheal modes bind exact dialog variables, visibility, fill and pulse sequences. | One immutable health binding emits `%Health%`, `%MaxHealth%`, bonus/death-warning mode and exact named animation edges. | Fixed threshold, overheal, death and recovery timelines compare panel and animation transcripts. | Ready |
| Active-weapon clip/no-clip/reserve state and strict low-ammo threshold bind `HudAmmoWeapons.res`. | One ammo binding preserves unavailable state, reload phase, label mode, float32/ties-to-even adjustment and exact low-ammo pulse edges. | Clip/reserve/threshold/reload/regenerate timelines compare every boundary. | Ready |
| Weapon selection retains ordered slot/position/item facts and never performs selection inside HUD presentation. | Immutable selection facts bind six resource slots; UI actions return typed selection requests only. | Selection transcript and transition-spy tests. | Ready |
| TF2 crosshair visibility is the conjunction of configured, player, weapon, client-view and TF2 suppression facts. | Crosshair values retain explicit unavailable state, texture/color/scale and supplied virtual-clock countdown suppression without querying browser state. | Eligibility decision table. | Ready |
| Damage messages append scale, lifetime and canonical direction in message order; missing direction appends nothing. | Ordered damage events emit typed indicator commands only when a supplied direction is available. | Direction-present/absent, clamp and coalesced-event vectors. | Ready |
| Nine class identities and RED/BLU team identities select exact class-image resources; five condition words select exact status images with buff-class coalescing. | Class/team and reviewed condition-panel inventories produce immutable image/visibility values. | All class/team images and every reviewed condition collision/ordering vector. | Ready |
| Death hides live status, publishes local respawn, and supplies freeze-panel killer facts independently. | Lifecycle values retain active/dying/observer state, explicit respawn eligibility and optional freeze facts; typed respawn action cannot mutate gameplay. | Death/respawn timeline and transition-spy tests. | Ready |
| Successful regeneration and pickup facts publish after authoritative state changes; TF2 does not draw generic pickup history. | Ordered typed notifications accompany resulting immutable health/ammo/condition facts; no pickup-history panel is invented. | Regenerate and health/ammo/weapon pickup transcripts. | Ready |
| Death notices retain ordered participant/team/icon/crit/assist/custom/local facts with a four-entry configured presentation bound. | Killfeed facts are copied into typed append commands; lifetime remains a generic virtual-clock concern. | Ordered typed notice and unavailable-producer vectors. | Ready |
| Scoreboard team, row and selected-player detail facts remain gameplay/replay facts rather than visual inference. | A bounded immutable scoreboard-ready snapshot retains exact team variables, rows and detail counters without sorting or scoring in VGUI. | 64-player bound, identity/order, unavailable-class and immutable-input tests. | Ready |
| Current compact Soldier/Demoman publications retain every selected simulation tick and ordered gameplay/lifecycle event. | An adapter maps compact identities and all per-tick health/ammo/reload/weapon/lifecycle/regenerate edges; absent damage direction, killfeed, pickup and scoreboard producers remain explicit. | Soldier fire/reload/death coalescing and Demoman class/team/overheal vectors. | Ready |
| HUD input produces typed selection, respawn and scoreboard requests only. | Pure action binding validates current immutable facts and returns frozen requests or explicit unavailability. | No gameplay transition call and input non-mutation tests. | Ready |

## Exclusions

- Generic VGUI resource parsing, layout, animation execution, DOM/CSS, focus and virtual-clock lifetime.
- Gameplay, replay decoding, settings persistence, app input routing and WASM/transport changes.
- Main menu, loading, pause, GameUI and Options behavior.

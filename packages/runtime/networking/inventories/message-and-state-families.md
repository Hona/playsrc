# Networking Message And State Families

Owning roadmap: [`../ROADMAP.md`](../ROADMAP.md).

## Inventory State

This is a candidate inventory, not generated output. It contains 92 candidate items and 0 generated or accepted items. Candidate items contribute 0 items to the Networking completion denominator until one checked-in generator emits this file and a denominator review accepts it.

| Metadata | Value |
|---|---|
| Authority identity | Valve Source SDK 2013 `usercmd`, prediction, network message, channel, data-table, string-table, event, and client-update interfaces; playsrc-owned transport-neutral connection and delivery contract |
| Authority revision | SDK commit `88fa198fba3fb85d46d4c95018254693fdc3af0a`; playsrc denominator review Missing |
| Generator command | Missing |
| Output path | `packages/runtime/networking/inventories/message-and-state-families.md` |
| Owning roadmap | `packages/runtime/networking/ROADMAP.md` |
| Candidate item count | 92 |
| Generated item count | 0 |
| Accepted item count | 0 |

Active checkpoint selection: protocol-24 packet bit framing; `server.send-table`, `server.class-info`, `server.string-table-create`, `server.string-table-update`, `server.game-event`, `server.user-message`, `server.packet-entities`, and `server.game-event-list`; all replicated property kinds and quantization/layout declarations needed by those messages; `snapshot.full`, `snapshot.delta`, `baseline.class`, and every `entity.*` state family. This recorded decode selection is Ready and does not accept live-session, encode, or delivery items.

## Connection Stages

The eight stages are ordered. Denial and closure are terminal results, not additional synchronization stages.

| Stable identity | Required observable state | Candidate result |
|---|---|---|
| `connection.none` | No peer, profile, schema, channel cursor, or session-owned state exists. | Candidate required |
| `connection.challenge` | Admission challenge identities and retry timing exist; sequenced payloads are rejected. | Candidate required |
| `connection.connected` | Admission succeeded and packet cursors exist; profile-dependent state is not synchronized. | Candidate required |
| `connection.metadata` | Server/session metadata and exact protocol profile are accepted. | Candidate required |
| `connection.schema` | Data tables, class bindings, game-owned schema identity, and initial string-table definitions are accepted. | Candidate required |
| `connection.ready` | Required content/session acknowledgements are complete and a full authoritative snapshot is required next. | Candidate required |
| `connection.active` | At least one valid full snapshot is authoritative; commands, deltas, events, and corrections may advance. | Candidate required |
| `connection.level-change` | Prior level state is unavailable; only declared transition and new-session synchronization messages are accepted. | Candidate required |

## Message Envelopes

The 42 candidate identities comprise 3 channel-control envelopes, 4 bidirectional common messages, 10 client-to-server messages, and 25 server-to-client messages. Stable identities use playsrc spelling; the corresponding official handler family is recorded where the SDK declares one. Payload semantics after bounded outer decoding belong to the owner in the final column.

| Stable identity | Direction | Official handler family | Semantic owner after Networking | Candidate result |
|---|---|---|---|---|
| `control.padding` | Bidirectional | Channel control | Networking | Candidate required |
| `control.disconnect` | Bidirectional | Channel control | Networking lifecycle | Candidate required |
| `control.file` | Bidirectional | Channel control | Content/application policy | Candidate required |
| `common.tick` | Bidirectional | `NET_Tick` | Networking synchronization | Candidate required |
| `common.string-command` | Bidirectional | `NET_StringCmd` | Application/game command policy | Candidate required |
| `common.convar-set` | Bidirectional | `NET_SetConVar` | Game/application configuration policy | Candidate required |
| `common.signon-state` | Bidirectional | `NET_SignonState` | Networking connection state | Candidate required |
| `client.client-info` | Client to server | `CLC_ClientInfo` | Application/session identity | Candidate required |
| `client.move` | Client to server | `CLC_Move` | Simulation command ingestion | Candidate required |
| `client.voice-data` | Client to server | `CLC_VoiceData` | Audio/application voice policy | Candidate required |
| `client.baseline-ack` | Client to server | `CLC_BaselineAck` | Networking baseline state | Candidate required |
| `client.listen-events` | Client to server | `CLC_ListenEvents` | Game event subscription | Candidate required |
| `client.cvar-response` | Client to server | `CLC_RespondCvarValue` | Application/game configuration policy | Candidate required |
| `client.file-crc-check` | Client to server | `CLC_FileCRCCheck` | Content/application integrity policy | Candidate required |
| `client.file-md5-check` | Client to server | `CLC_FileMD5Check` | Content/application integrity policy | Candidate required |
| `client.save-replay` | Client to server | `CLC_SaveReplay` | Replay/application policy | Candidate required |
| `client.command-keyvalues` | Client to server | `CLC_CmdKeyValues` | Game/application command policy | Candidate required |
| `server.print` | Server to client | `SVC_Print` | Application console/presentation | Candidate required |
| `server.info` | Server to client | `SVC_ServerInfo` | Networking metadata plus application map selection | Candidate required |
| `server.send-table` | Server to client | `SVC_SendTable` | Networking schema registry | Candidate required |
| `server.class-info` | Server to client | `SVC_ClassInfo` | Networking schema registry | Candidate required |
| `server.pause` | Server to client | `SVC_SetPause` | Simulation/application pause policy | Candidate required |
| `server.string-table-create` | Server to client | `SVC_CreateStringTable` | Networking table registry | Candidate required |
| `server.string-table-update` | Server to client | `SVC_UpdateStringTable` | Networking table registry | Candidate required |
| `server.voice-init` | Server to client | `SVC_VoiceInit` | Audio/application voice policy | Candidate required |
| `server.voice-data` | Server to client | `SVC_VoiceData` | Audio/application voice policy | Candidate required |
| `server.sounds` | Server to client | `SVC_Sounds` | Audio presentation | Candidate required |
| `server.set-view` | Server to client | `SVC_SetView` | Game/presentation view policy | Candidate required |
| `server.fix-angle` | Server to client | `SVC_FixAngle` | Game prediction/presentation policy | Candidate required |
| `server.crosshair-angle` | Server to client | `SVC_CrosshairAngle` | Game presentation policy | Candidate required |
| `server.bsp-decal` | Server to client | `SVC_BSPDecal` | Rendering presentation | Candidate required |
| `server.game-event` | Server to client | `SVC_GameEvent` | Game/replay event interpretation | Candidate required |
| `server.user-message` | Server to client | `SVC_UserMessage` | Game-owned message registry | Candidate required |
| `server.entity-message` | Server to client | `SVC_EntityMessage` | Game-owned entity message registry | Candidate required |
| `server.packet-entities` | Server to client | `SVC_PacketEntities` | Networking replicated state | Candidate required |
| `server.temp-entities` | Server to client | `SVC_TempEntities` | Game/presentation temporary entities | Candidate required |
| `server.prefetch` | Server to client | `SVC_Prefetch` | Content and presentation | Candidate required |
| `server.menu` | Server to client | `SVC_Menu` | Application UI | Candidate required |
| `server.game-event-list` | Server to client | `SVC_GameEventList` | Networking event schema registry | Candidate required |
| `server.get-cvar` | Server to client | `SVC_GetCvarValue` | Application/game configuration policy | Candidate required |
| `server.command-keyvalues` | Server to client | `SVC_CmdKeyValues` | Game/application command policy | Candidate required |
| `server.pause-timed` | Server to client | `SVC_SetPauseTimed` | Simulation/application pause policy | Candidate required |

## Replicated Property Kinds

| Stable identity | Required schema declaration | Candidate result |
|---|---|---|
| `property.integer` | Signedness, bit count or varint mode, and accepted scalar domain. | Candidate required |
| `property.float` | Bit count, low/high range, and one compatible float encoding declaration. | Candidate required |
| `property.vector3` | Per-component float declaration and optional normal reconstruction. | Candidate required |
| `property.vector2` | Per-component float declaration with an absent third component. | Candidate required |
| `property.string` | Maximum encoded bytes and exact byte/string interpretation. | Candidate required |
| `property.array` | Element property identity, declared maximum elements, and encoded current count. | Candidate required |
| `property.table` | Child-table identity, flattening behavior, exclusion behavior, and recipient rule. | Candidate required |

## Quantization And Layout Declarations

Each field accepts only declarations compatible with its property kind. `encoding.normal-or-varint` is one encoded flag position whose meaning is selected by property kind; the generator must reject an ambiguous use.

| Stable identity | Required observable effect | Candidate result |
|---|---|---|
| `encoding.unsigned` | Integer values use an unsigned domain. | Candidate required |
| `encoding.world-coordinate` | Float components use the declared world-coordinate representation. | Candidate required |
| `encoding.no-scale` | Float bits are retained without range scaling. | Candidate required |
| `encoding.round-down` | Quantization excludes the high endpoint by one encoded unit. | Candidate required |
| `encoding.round-up` | Quantization excludes the low endpoint by one encoded unit. | Candidate required |
| `encoding.normal-or-varint` | Vector/float normal encoding or integer varint encoding is selected solely by property kind. | Candidate required |
| `encoding.exclude` | The named property from the named table is removed during flattening. | Candidate required |
| `encoding.xyze` | Vector components use the declared shared-exponent representation. | Candidate required |
| `encoding.inside-array` | The property is encoded only through its owning array declaration. | Candidate required |
| `encoding.proxy-always` | The nested table is eligible for every recipient supplied by its owner. | Candidate required |
| `encoding.changes-often` | Flattened property order moves the field into the stable frequently-changing prefix. | Candidate required |
| `encoding.vector-element` | The field is one explicitly indexed component of a vector declaration. | Candidate required |
| `encoding.collapsible` | A nested table contributes fields without retaining its table node in flattened order. | Candidate required |
| `encoding.multiplayer-coordinate` | Float components use the multiplayer coordinate representation. | Candidate required |
| `encoding.multiplayer-coordinate-low` | Float components use reduced fractional precision in the multiplayer coordinate representation. | Candidate required |
| `encoding.multiplayer-coordinate-integral` | Float components use integral multiplayer coordinate representation. | Candidate required |
| `encoding.tick-relative` | The field's encoded value is interpreted relative to the authoritative snapshot tick. | Candidate required |

## Snapshot And Entity State Families

| Stable identity | Required state transition | Candidate result |
|---|---|---|
| `snapshot.full` | Construct one complete authoritative snapshot without a prior snapshot. | Candidate required |
| `snapshot.delta` | Construct one authoritative snapshot from one exact retained base. | Candidate required |
| `baseline.class` | Initialize a newly created class instance from immutable class-owned state. | Candidate required |
| `baseline.rolling` | Maintain two acknowledged rolling entity-state slots with explicit identities. | Candidate required |
| `entity.create` | Bind slot, serial, class, baseline, and initial field delta atomically. | Candidate required |
| `entity.update` | Apply the ordered declared field delta to an existing matching slot/serial/class. | Candidate required |
| `entity.preserve` | Carry the prior matching entity unchanged into the next snapshot. | Candidate required |
| `entity.leave-relevance` | Remove the entity from the recipient snapshot without declaring destruction. | Candidate required |
| `entity.delete` | Destroy the matching entity identity and invalidate its recipient state. | Candidate required |

## Game Event Field Kinds

`event.local` is retained for local dispatch but never enters a network payload.

| Stable identity | Required value | Candidate result |
|---|---|---|
| `event.local` | Owner-defined local value excluded from network encoding. | Candidate required |
| `event.string` | Bounded exact string value. | Candidate required |
| `event.float` | IEEE-754 binary32 value under the event schema's comparison rule. | Candidate required |
| `event.integer` | Signed integer value under the declared event encoding. | Candidate required |
| `event.uint64` | Unsigned 64-bit integer value. | Candidate required |
| `event.wstring` | Bounded wide-string value under the declared code-unit contract. | Candidate required |
| `event.boolean` | Boolean value. | Candidate required |

## Delivery Classes

| Stable identity | Required disposition | Candidate result |
|---|---|---|
| `delivery.reliable` | Retain, fragment when required, retransmit until acknowledged or terminal failure, and deliver once in order. | Candidate required |
| `delivery.unreliable` | Send at most once, admit only as a complete message, and never delay newer replaceable state to recover loss. | Candidate required |

## Generation Contract

The future generator must consume the pinned SDK declarations, the accepted root Owner Registry, the accepted Networking roadmap, and the accepted TF2 game-owned networking inventory. It emits these seven sections and every stable identity in the exact order declared here, records direction, delivery class, and semantic owner for every message, and emits exactly 92 package-owned items unless the same reviewed change updates the roadmap denominator.

Generation fails on a duplicate identity, changed official handler assignment, missing owner, invalid direction, absent delivery class, ambiguous property/encoding pairing, unclassified event field kind, unclassified entity transition, or item-count mismatch. Failed, missing, unknown, unsupported, and owner-conflict discoveries remain visible in generator diagnostics and are never silently omitted.

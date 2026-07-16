# TF2 Particle System Inventory

Owning roadmap: [`../ROADMAP.md`](../ROADMAP.md)

## Inventory Metadata

| Field | Value |
|---|---|
| Authority identity | TF2 content build `10822003`; `particles/particles_manifest.txt`; exact `tf2_misc_dir.vpk` index; 233 configured TF2 BSP files and their PAK indexes; 85 selected BSP-PAK map manifests; one selected GAME map manifest; every available selected PCF byte sequence |
| Authority revision | `steam.inf` SHA-256 `b8d7c1eb4517a806d514087facf42e3d8f407bf14393ac5fdc5d4c69e40adc7f`; global manifest SHA-256 `fde462e2fe756e47688e77df851c79dab27bfe2c977724c649b7170b99b8a85b`; VPK index SHA-256 `63f7db0d1c509e303ca9002fee9e3d805e9220ea5afdd639d8a6b68b8a3710b9`; nine selected PCF revisions Missing |
| Generator command | Missing |
| Output path | `packages/presentation/particle/inventories/tf2-systems.md` |
| Item count | 12,651 available Candidate system definitions; complete count Unknown; 0 Accepted |

This file is a manually derived Candidate and is not a generated inventory. Its items contribute 0 completion-denominator items until a checked-in generator emits one stable row per definition, all selected PCF references resolve, the output is current, and a denominator review satisfies [`../../../../docs/roadmap-contract.md`](../../../../docs/roadmap-contract.md).

One available Candidate item has stable identity `sha256:<PCF bytes>/dmx:<definition UUID>`. The 12,651-item available set has SHA-256 `8ac42b617639d85f0087321e8f75ab521c1e042a74f01eae7f953f4c3722676e` when sorted by `(PCF SHA-256, UUID)` and serialized as lowercase hash, tab, lowercase UUID, newline. Exactly 12,525 items are effective in at least one global-or-map profile; that subset has SHA-256 `02dff36f165a21c6e045f8183c78c94f253325311bb355e28d7c6c6a1aa55874`. The remaining 126 available items are shadowed in every configured profile.

## Resolution Summary

- The global manifest has 106 entries, 105 distinct logical paths, 104 precache-marked entries, one repeated `particles/buildingdamage.pcf` entry, and one Missing `particles/error.pcf` entry.
- The 104 available distinct global PCFs contain 9,125 definition occurrences. Manifest-order name/UUID resolution leaves 9,049 effective global definitions: 9,048 name-indexed definitions and one UUID-only definition. Fifty-four ASCII-insensitive names have prior occurrences; those names account for 76 shadowed definition occurrences.
- The configured map set contains 233 BSPs. Eighty-five BSP PAKs contain the selected map-manifest identity, four of those manifests contain no `file` entry, and 148 BSPs contain neither PAK identity. Exact GAME lookup additionally selects `maps/cp_coldfront_particles.txt`, SHA-256 `2ba052b419585c38cf93e57bb691ba5772cf949969dfed18de7541e3af052b33`.
- The 86 selected map manifests contain 173 ordered PCF references: 161 resolve in the active map PAK, three resolve in `tf2_misc_dir.vpk`, and nine are Missing. The 161 PAK resolutions select 129 distinct PCF byte sequences containing 4,056 definition items before cross-source deduplication. The GAME manifest selects the already global `particles/stormfront.pcf` bytes and adds no Candidate definition identity.
- The global and map partitions contain 13,181 definition occurrences across 233 source partitions and 228 distinct PCF byte sequences. Deduplication by PCF SHA-256 plus definition UUID yields the 12,651 available Candidate items.
- The largest available effective profile is `pd_watergate` with 9,405 definitions. The global-only profile and every map with no selected additions contain 9,049.

## Missing Selected PCFs

| Logical path | Map profiles | Checked providers | Classification |
|---|---|---|---|
| `particles/fire_01.pcf` | `arena_lumberyard_event`, `cp_cowerhouse`, `koth_demolition`, `vsh_outburst` | Each active BSP PAK, then `tf2_misc_dir.vpk` | Missing |
| `particles/env_snow_128_loose.pcf` | `ctf_sidewinder` | Active BSP PAK, then `tf2_misc_dir.vpk` | Missing |
| `particles/env_snow_256_loose.pcf` | `ctf_sidewinder` | Active BSP PAK, then `tf2_misc_dir.vpk` | Missing |
| `particles/env_snow_1024_loose.pcf` | `ctf_sidewinder` | Active BSP PAK, then `tf2_misc_dir.vpk` | Missing |
| `particles/explosion_high_copy.pcf` | `plr_hacksaw` | Active BSP PAK, then `tf2_misc_dir.vpk` | Missing |
| `particles/hightower_explosion.pcf` | `vsh_nucleus` | Active BSP PAK, then `tf2_misc_dir.vpk` | Missing |

## Selected Map Manifests

| Map profile | Provider | Manifest identity | SHA-256 | Entries |
|---|---|---|---|---:|
| `2koth_abbey` | BSP PAK | `maps/2koth_abbey_particles.txt` | `04e4d6a296948b254a49261b229e74f4f30d4b1cb1ffe6697ac86db062eff7be` | 1 |
| `arena_afterlife` | BSP PAK | `maps/arena_afterlife_particles.txt` | `692bb163555fec2daf56561e58e5530ecb1584d63af965864754f417f89d2c24` | 1 |
| `arena_byre` | BSP PAK | `maps/arena_byre_particles.txt` | `7dd09c5c613c18cc28a59fd1ee7c9841efe690ec9ec99f9307e8cbf616749d4a` | 1 |
| `arena_lumberyard_event` | BSP PAK | `maps/arena_lumberyard_event_particles.txt` | `6687119c568e019fb4871c76eb3ed7af6418005ec30b4047ab27a279a3c11ef9` | 6 |
| `cp_brew` | BSP PAK | `maps/cp_brew_particles.txt` | `ccde5b96002822c5cca05134de3341c3b53b572be6c9beae5a28d73d31190dab` | 1 |
| `cp_canaveral_5cp` | BSP PAK | `maps/cp_canaveral_5cp_particles.txt` | `e0b240bfe9aa987742f04ce863954be0956e76f8261a1121305dcd278c4877a2` | 1 |
| `cp_coldfront` | GAME VPK | `maps/cp_coldfront_particles.txt` | `2ba052b419585c38cf93e57bb691ba5772cf949969dfed18de7541e3af052b33` | 1 |
| `cp_cowerhouse` | BSP PAK | `maps/cp_cowerhouse_particles.txt` | `e1a122e7952ede4dcb1e66496bd8d52074a35b33cad68372b06be427a88a6e56` | 1 |
| `cp_darkmarsh` | BSP PAK | `maps/cp_darkmarsh_particles.txt` | `7b1324cbe43eea5e5be185e8d54d61a187fb364ba9da6b9d05db508715ff8f04` | 1 |
| `cp_degrootkeep_rats` | BSP PAK | `maps/cp_degrootkeep_rats_particles.txt` | `43f32ccf694a2cfda2cfff6986d34eb646d896fabeefb70634cac94e10e6d5a1` | 1 |
| `cp_frostwatch` | BSP PAK | `maps/cp_frostwatch_particles.txt` | `67272c8b764dfb5e117c65441e6660e37a52b5502cfe39cbc32f60db39ff0f73` | 1 |
| `cp_fulgur` | BSP PAK | `maps/cp_fulgur_particles.txt` | `005dc3462c000d247b2b9a6623e2b717e2c67218f9b815f6820f585130fd1532` | 1 |
| `cp_gravelpit_snowy` | BSP PAK | `maps/cp_gravelpit_snowy_particles.txt` | `567c6ce96c530edd48fb0965c7cc6c92ad34f6c79dc30af20b8bbed9d5adfd09` | 1 |
| `cp_hadal` | BSP PAK | `maps/cp_hadal_particles.txt` | `e2df6fee4fe9b79cd87197fdd987db804e734607377bb6c8e304f908c9a93eca` | 1 |
| `cp_premuda` | BSP PAK | `maps/cp_premuda_particles.txt` | `c450b18343578defa65abec7ab0552a12bfbcb9e8f11f0d0d38b6dbdc3b8bbd5` | 1 |
| `cp_snowplow` | BSP PAK | `maps/cp_snowplow_particles.txt` | `1e8de5ad26d692431cd0f1b6ff92478e73675cd6edb305435b15a27b965a3d21` | 7 |
| `cp_spookeyridge` | BSP PAK | `maps/cp_spookeyridge_particles.txt` | `67fd0d96ac8541b25e1bed8fab6302cf4f4826a0adb7886272c70bd56514140e` | 0 |
| `cp_sulfur` | BSP PAK | `maps/cp_sulfur_particles.txt` | `394ad54417adf91d902af2016c15ca6150ad3317538e4d212175ab1b66ebfd85` | 0 |
| `cppl_gavle` | BSP PAK | `maps/cppl_gavle_particles.txt` | `0af2374b23a81ab34af068ea164c4c9d948b932b0dca2e43b8ebc0e6152947a1` | 2 |
| `ctf_2fort_invasion` | BSP PAK | `maps/ctf_2fort_invasion_particles.txt` | `c7e80eee2fc88535f49da9adc9c7de745c265009796d69bec40175554a7330c2` | 1 |
| `ctf_crasher` | BSP PAK | `maps/ctf_crasher_particles.txt` | `b7bef9825c35c9c10013245ca9d7cfb6944737eadd810760f5ee56d05857f082` | 2 |
| `ctf_doublecross_event` | BSP PAK | `maps/ctf_doublecross_event_particles.txt` | `f383c9b1224c1bc89c9542c930e5812eb004a0ae743b5fc04b3f9a13708b8e23` | 5 |
| `ctf_haarp` | BSP PAK | `maps/ctf_haarp_particles.txt` | `be8b426a26fff2c32c5a316191172d21271eac3f0533e0bf81294339e3757704` | 1 |
| `ctf_helltrain_event` | BSP PAK | `maps/ctf_helltrain_event_particles.txt` | `5d7d4d42ea09c78c89ad1cae595fa938eeb1dcc85831700c299cb87d7f2cbb81` | 5 |
| `ctf_pressure` | BSP PAK | `maps/ctf_pressure_particles.txt` | `44219aa220a3a69ac54044f9ab985760f59b9bfc17ce273cd0dbbff966001582` | 2 |
| `ctf_sidewinder` | BSP PAK | `maps/ctf_sidewinder_particles.txt` | `2b2ada02b342a8abc4f441b6429cc178bd8af6c29d88b834042ea2e91a0cb60a` | 3 |
| `ctf_snowfall_final` | BSP PAK | `maps/ctf_snowfall_final_particles.txt` | `bfeac1cbb90a776b8130cd7089ef05b482a11c873f36718aa36a431148d19364` | 2 |
| `htf_marshlands` | BSP PAK | `maps/htf_marshlands_particles.txt` | `1931de1c46f3d0593f4e602bcc431e9729f07b7d6c074bbba2b541207fa85bb4` | 1 |
| `koth_bagel_event` | BSP PAK | `maps/koth_bagel_event_particles.txt` | `7c7160e6f22df077e9427756a545e9a5125ff402cc53ce4bb7d514b1bc1be546` | 1 |
| `koth_blowout` | BSP PAK | `maps/koth_blowout_particles.txt` | `9f86ef2203274062f33d5c5602987a215c2eae0840b5537817d3a9271a3ef170` | 1 |
| `koth_boardwalk` | BSP PAK | `maps/koth_boardwalk_particles.txt` | `b508fe67b9ff0468e60a5cab10f6c74ff7463a5b22a64152cd86ae5d06477c50` | 3 |
| `koth_cachoeira` | BSP PAK | `maps/koth_cachoeira_particles.txt` | `c7de18e157c54d015c9ea556af2c754ab91d33a4c3d4bcca003d6221492f6c53` | 3 |
| `koth_demolition` | BSP PAK | `maps/koth_demolition_particles.txt` | `3744422902d590cb9e83ae329e1895f3df3fe16554d9d92c2b254119a436604a` | 1 |
| `koth_dusker` | BSP PAK | `maps/koth_dusker_particles.txt` | `3a6e196e82eed8a810afd2a377f146ab938141677eae9570cd7d076534e0ed20` | 1 |
| `koth_krampus` | BSP PAK | `maps/koth_krampus_particles.txt` | `13f250829571f476fc22cd247320ede954c25be0453f9761cd833081a6cadc8d` | 2 |
| `koth_los_muertos` | BSP PAK | `maps/koth_los_muertos_particles.txt` | `fad607e02c7acf7776e893d0f1f36d41dfa0c9da5fe192801e0c164a79b8d7b8` | 1 |
| `koth_mannhole` | BSP PAK | `maps/koth_mannhole_particles.txt` | `4410ecfe86112ec16bdfbbf065582c685eb1c9e6a8cd75b3785fd543c4e9f03b` | 1 |
| `koth_megalo` | BSP PAK | `maps/koth_megalo_particles.txt` | `83bf3fba174f5c0417d8f81c4e35bcc113c00f9d0e545e7566af7121e34b1b91` | 3 |
| `koth_megaton` | BSP PAK | `maps/koth_megaton_particles.txt` | `20684d093db52306988428a263505c2d394e1dea3a49c048d68764aa33da9191` | 1 |
| `koth_probed` | BSP PAK | `maps/koth_probed_particles.txt` | `00a62e652b1cc32fcc6ca38ea47747fcae638363c6078b362d9f674a17649e04` | 2 |
| `koth_sawmill_event` | BSP PAK | `maps/koth_sawmill_event_particles.txt` | `460dd0b528d5122a8d3c6121b37832a6087570d63a88dda1060fb66c33558b7b` | 1 |
| `koth_slasher` | BSP PAK | `maps/koth_slasher_particles.txt` | `45dd72d75d76e076fac0a3739d965def7b8af07e7d4d921653df23b66dd18f52` | 1 |
| `koth_slaughter_event` | BSP PAK | `maps/koth_slaughter_event_particles.txt` | `9d4378e2b8703763afb03081606f49be1439c4dd13ab6ca6793a5f9f17db192a` | 2 |
| `koth_slime` | BSP PAK | `maps/koth_slime_particles.txt` | `5789eeeb6f620cfeb047eb47a4643090b27e8a62c25156a16365ddae58dee542` | 2 |
| `koth_snowtower` | BSP PAK | `maps/koth_snowtower_particles.txt` | `2dd08870c490712f16c8efce3c39297a49eafb1ab42642832fdc459d31ce115e` | 1 |
| `koth_suijin` | BSP PAK | `maps/koth_suijin_particles.txt` | `20d7b050193d2df099fdecd59c1320d68f5f82289af254fc1c9164ca0eccb12c` | 0 |
| `koth_toxic` | BSP PAK | `maps/koth_toxic_particles.txt` | `c5e7044881fbf91cf01181ebd6cb4edc4b812ca94417be6f4f1ee2aee3ac1ed2` | 4 |
| `koth_undergrove_event` | BSP PAK | `maps/koth_undergrove_event_particles.txt` | `cf5af71a857a3d3b6902f08caae142c44329cbec79f77c6e616ad30e79cf7b8a` | 1 |
| `pd_atom_smash` | BSP PAK | `maps/pd_atom_smash_particles.txt` | `1b0dfcba7fa2e0d3791af614f62bd5d3dd16d0e83c91efce09715a9276ff359b` | 2 |
| `pd_circus` | BSP PAK | `maps/pd_circus_particles.txt` | `d1ea736d8945a8dd74aae54f3e6a605ff2f42c0b4eb82cfc07dff065cc389c7d` | 4 |
| `pd_cursed_cove_event` | BSP PAK | `maps/pd_cursed_cove_event_particles.txt` | `be21b74bf84166fe9795d6b40ecfb7596ee25c066885c5970ec9135ff80a89f5` | 1 |
| `pd_farmageddon` | BSP PAK | `maps/pd_farmageddon_particles.txt` | `c040642731596b94ab79a6d9024524d2242dc9e2f284127042243f3e8c3317df` | 2 |
| `pd_galleria` | BSP PAK | `maps/pd_galleria_particles.txt` | `7097c61cb9b2fb5140d24bce4a3f64401d346d67c237c7fc5ec0cefc75cc2297` | 1 |
| `pd_monster_bash` | BSP PAK | `maps/pd_monster_bash_particles.txt` | `321b7376859c0dfe3e856d60a020723e11b8e9e33a8b098ad43bf4df0ccec176` | 1 |
| `pd_pit_of_death_event` | BSP PAK | `maps/pd_pit_of_death_event_particles.txt` | `e97366b2171d64c486163643a02ab1970918fafb88d1964f0ee324aec6b98755` | 0 |
| `pd_snowville_event` | BSP PAK | `maps/pd_snowville_event_particles.txt` | `c4f06469557c765f4bac1a2369e8956cde67d381090a3baa4b1bf3e9ef66e062` | 1 |
| `pd_watergate` | BSP PAK | `maps/pd_watergate_particles.txt` | `0fcbc3506adf2bc7c5858c1ccc6f7a1de4c3e89050f00d612b28d56859c2d42f` | 9 |
| `pl_aquarius` | BSP PAK | `maps/pl_aquarius_particles.txt` | `d0a265ef8ae72c1f7c81d30cb6e0f84e76139c885d20aa90e7bbe4286c5fa297` | 2 |
| `pl_breadspace` | BSP PAK | `maps/pl_breadspace_particles.txt` | `70fc12235dd537283c6ee995822f8847c43a61103278d18e9d154c1b9b0eea64` | 1 |
| `pl_coal_event` | BSP PAK | `maps/pl_coal_event_particles.txt` | `012f1972f89d3e11cdee5437c73b672d3f1b1e93da1b107403b841827af204c6` | 1 |
| `pl_corruption` | BSP PAK | `maps/pl_corruption_particles.txt` | `ac21697b2eca03c955a8dc54e0cef203793772f0dfe82a8c6b50528b828df85f` | 2 |
| `pl_embargo` | BSP PAK | `maps/pl_embargo_particles.txt` | `222f72130b59d4a1b214640d13ef3653613acbe5902c984490053907665e9cf3` | 1 |
| `pl_frostcliff` | BSP PAK | `maps/pl_frostcliff_particles.txt` | `7d3ac6f319372b3a8cb57f5c93cc995326e168f6d35749fc2538c102803024af` | 2 |
| `pl_patagonia` | BSP PAK | `maps/pl_patagonia_particles.txt` | `9f16a9287af04269780b3d4fd793be65afd578e24d33a53c857df7f17be628f4` | 1 |
| `pl_phoenix` | BSP PAK | `maps/pl_phoenix_particles.txt` | `87f862960322476f3985f808971bab28ce20cb49e0e0c229c495cbf13dfa56ba` | 1 |
| `pl_pier` | BSP PAK | `maps/pl_pier_particles.txt` | `2f55030d2d90e2bf07c0335edfa52b838920c78b4c03472e9724334177ed127e` | 1 |
| `pl_precipice_event_final` | BSP PAK | `maps/pl_precipice_event_final_particles.txt` | `175c479e0294439c96e131c3332f4f8e7d6b14398ccd2c7159f35d3f87fb3ee0` | 1 |
| `pl_rumble_event` | BSP PAK | `maps/pl_rumble_event_particles.txt` | `ff3d9e5cffb2e2f1970815b6140a7cd05b7686ff1a260d3db65b91072a0ce92d` | 3 |
| `pl_rumford_event` | BSP PAK | `maps/pl_rumford_event_particles.txt` | `6fdfff7b1c14438a34935e2f7daf5bf25d23e737aca44c5b5f2df95ee152dbb6` | 2 |
| `pl_snowycoast` | BSP PAK | `maps/pl_snowycoast_particles.txt` | `b316478d5c5b6bf58e017d2de23bf6e6bf4aee8878a80f6e9b2c45659903a911` | 1 |
| `plr_hacksaw` | BSP PAK | `maps/plr_hacksaw_particles.txt` | `6561a5cab7c25fe627a2f4d2ff792b62687b9b7d258d80ab490f1306d33d715f` | 5 |
| `plr_hacksaw_event` | BSP PAK | `maps/plr_hacksaw_event_particles.txt` | `caddb0a3a296b0e371031d01451811ac31176465aa021b11b0e4e18322e3b1c0` | 6 |
| `plr_matterhorn` | BSP PAK | `maps/plr_matterhorn_particles.txt` | `a9565f3ae636e49963083e86dc4b88010ca20532a0c44311fd1a26b77421ceea` | 3 |
| `tow_dynamite` | BSP PAK | `maps/tow_dynamite_particles.txt` | `69b592848956bf45be015f77577e27eecdaf263304eb64b44f26d841e7a27690` | 1 |
| `vsh_distillery` | BSP PAK | `maps/vsh_distillery_particles.txt` | `dac53aba671804e0ae5d20707e8da91f0696bfb0ea297593f06210321a3c43d2` | 3 |
| `vsh_maul` | BSP PAK | `maps/vsh_maul_particles.txt` | `515a5b534cd79bdc7e89345a111a2450e285c688f78ad94b31ee32b47a8f9164` | 4 |
| `vsh_nucleus` | BSP PAK | `maps/vsh_nucleus_particles.txt` | `58979fe403479182212c693cbc1c16892f84e5f64f111d8e7557e1ea84e62c04` | 6 |
| `vsh_outburst` | BSP PAK | `maps/vsh_outburst_particles.txt` | `3934996e33f140c5b543d278f35ffd9b3005dfda3c10f5b1ae8c77682c0cf06c` | 5 |
| `vsh_skirmish` | BSP PAK | `maps/vsh_skirmish_particles.txt` | `dac53aba671804e0ae5d20707e8da91f0696bfb0ea297593f06210321a3c43d2` | 3 |
| `vsh_tinyrock` | BSP PAK | `maps/vsh_tinyrock_particles.txt` | `dac53aba671804e0ae5d20707e8da91f0696bfb0ea297593f06210321a3c43d2` | 3 |
| `zi_atoll` | BSP PAK | `maps/zi_atoll_particles.txt` | `094cd004bec38d186b78120757203d903ee74a1d30249f4062f57d6b702d85bc` | 1 |
| `zi_blazehattan` | BSP PAK | `maps/zi_blazehattan_particles.txt` | `3a05688f83f5644f7338d55ad202bb16b3d88737d174d77a1b2557511d8eaf93` | 2 |
| `zi_devastation_final1` | BSP PAK | `maps/zi_devastation_final1_particles.txt` | `8e1204be5a8aa201bbdb61ec79d0d4232fb79aa9df330143a3a3b14e9b90de9f` | 3 |
| `zi_murky` | BSP PAK | `maps/zi_murky_particles.txt` | `5d0f976759e48d3c8087004b54edac31cc3184f7700e5de14f7b4eb32f6b12ca` | 1 |
| `zi_sanitarium` | BSP PAK | `maps/zi_sanitarium_particles.txt` | `7e6455f6eb003b892d5eda305dbf87631f9e69b0132cc18a16900815ddd9e0ac` | 3 |
| `zi_woods` | BSP PAK | `maps/zi_woods_particles.txt` | `094cd004bec38d186b78120757203d903ee74a1d30249f4062f57d6b702d85bc` | 1 |

## Global PCF Source Partitions

`Definitions` counts source elements. `Effective` counts items selected by the complete global manifest after later name replacements. These rows partition global source occurrences; stable definition items are emitted only by the future generator.

| Manifest logical path | PCF SHA-256 | Definitions | Effective | Shadowed | State |
|---|---|---:|---:|---:|---|
| `particles/error.pcf` | Missing | 0 | 0 | 0 | Missing |
| `particles/rockettrail.pcf` | `d6141fed629c3df3a6f4db190fe47fbbbf017662220d5804538edef453f42868` | 53 | 52 | 1 | Candidate |
| `particles/smoke_blackbillow.pcf` | `5a41fe584484243a1442e6e17d1c05cf961c1b6312c04ae3ed38e01be554fb1d` | 27 | 27 | 0 | Candidate |
| `particles/teleport_status.pcf` | `a6b900d489b6c5ab27041fe93be624c03bf1250b73db34142283200ce3f9f5dd` | 40 | 40 | 0 | Candidate |
| `particles/explosion.pcf` | `f0ebb89371c85113bdad6dc65fae0d484b1a1678f80e14ecea991ff2aaa13606` | 66 | 56 | 10 | Candidate |
| `particles/player_recent_teleport.pcf` | `bbcfb5fa472e05b20f34779013a5688b017d8f0582c775e932a62a53801db161` | 10 | 10 | 0 | Candidate |
| `particles/rocketjumptrail.pcf` | `ab943b1282df5d3b872f78946821d433d3aee301addb784ac3af46953936eb17` | 11 | 11 | 0 | Candidate |
| `particles/rocketbackblast.pcf` | `a98a873842139ef24fac0331c2c8688478a85c62132ac8e86db6877babf0437f` | 2 | 2 | 0 | Candidate |
| `particles/flamethrower.pcf` | `dd9a429be649228cc8292076a38fff7814281600fd1a62732904f3d0dae6fe35` | 147 | 145 | 2 | Candidate |
| `particles/flamethrower_mvm.pcf` | `c092b82cb8b4f4e7461d4cf9fa47fcb70fed2e4f604a166e457c17d468974bb2` | 12 | 12 | 0 | Candidate |
| `particles/burningplayer.pcf` | `fbbbb834768f931b85019213adaa4502c03a1ba917fde8835801ea5db300b045` | 59 | 59 | 0 | Candidate |
| `particles/blood_impact.pcf` | `f9af5a9e2d8dd80b0c5e9b785d7f2fd4e32696a84a4b1e938f952c3fc95f7c21` | 44 | 43 | 1 | Candidate |
| `particles/blood_trail.pcf` | `e564f7b00b2bf13af2c602e1b9455cd822ea4a6af2b8e320290b36351c573123` | 2 | 2 | 0 | Candidate |
| `particles/muzzle_flash.pcf` | `dae62e42e4e556b9b54073633870a7228f90155e45e3bbdf4006829c0eaef27b` | 35 | 35 | 0 | Candidate |
| `particles/teleported_fx.pcf` | `9f06f5eec3df167f103da9ca73003fd0032fdf84354ebe08a6aeb86b556729c9` | 31 | 31 | 0 | Candidate |
| `particles/cig_smoke.pcf` | `2454826a30fdad4634b72aa2732d7a46aa955f55e8ffe6e0d4563174232ee2fa` | 3 | 3 | 0 | Candidate |
| `particles/crit.pcf` | `dc961e8fa5a39b5f4a3b18cc2629379dd2e2cc8e2340f9bccb49ea666ef28c6f` | 10 | 8 | 2 | Candidate |
| `particles/medicgun_beam.pcf` | `d70e9ab041891cdf2df73416e96ce550eed190cd3f0ee3c2e51460b7cb1ba557` | 68 | 68 | 0 | Candidate |
| `particles/bigboom.pcf` | `3d5ea98a38b36a28c36e7b3a8a8390c8603799c47dd257590ae7b7bb074ab18a` | 42 | 35 | 7 | Candidate |
| `particles/water.pcf` | `fe0b8f359c6fefcda9b664dc8ef0f2cecc75326823e2746832e986bba377625d` | 42 | 42 | 0 | Candidate |
| `particles/stickybomb.pcf` | `8496f1d12cd6cedff8f3db79f8df3c545184110333a9755f3cedf64ad3b303cf` | 20 | 20 | 0 | Candidate |
| `particles/buildingdamage.pcf` | `2bab9492a78193f05e7ccfa0231bd0ff187bd82133d40cfa92355cb38bcd595b` | 30 | 30 | 0 | Candidate; manifest entry occurs twice |
| `particles/nailtrails.pcf` | `58eac9fc060ab538896398d2618f27f86ece54018cfc40ec958689d845a0fce7` | 4 | 4 | 0 | Candidate |
| `particles/speechbubbles.pcf` | `e0b2301925325926d878aa3a5db7cbdf2d2ed9b48dc7fa0f867e555fa1ff8ee7` | 17 | 14 | 3 | Candidate |
| `particles/bullet_tracers.pcf` | `4bcc51838db7ddf0d1bb4052df4ffaa22096fbcbaf2f9d05b8e024bfdefb0428` | 34 | 34 | 0 | Candidate |
| `particles/nemesis.pcf` | `3b8d0e8229785d8fc4c26323b0f8f012d281ffea7d2224a6baa4ede128969ada` | 4 | 4 | 0 | Candidate |
| `particles/disguise.pcf` | `7886056584785184a3cd26e19b830026bf54fde01d2ad48191ae1a9293c420f0` | 8 | 8 | 0 | Candidate |
| `particles/sparks.pcf` | `56b1c41a6049a4bbd2e235917f0ce8b29aa677118f90176f1f5b2ec31b4dbaa2` | 22 | 22 | 0 | Candidate |
| `particles/flag_particles.pcf` | `0d9d9b8c878d8c99363739ec24688f6551fac3f598c795b378b3f2e346d2c5e0` | 12 | 12 | 0 | Candidate |
| `particles/shellejection.pcf` | `fce5a836a9dfa25401b269c2da9ba0e1dee7579e78e284f3d10089e40ed9a99b` | 1 | 1 | 0 | Candidate |
| `particles/medicgun_attrib.pcf` | `78306b7dfc5b4b3230fdb8e7025d6503c768fc5d3d6c74caaf2758dc56c8aaa9` | 10 | 10 | 0 | Candidate |
| `particles/item_fx.pcf` | `600fbacf130eb763c2939f3c7149ba0501804c55fed86f78be358e05b239ab72` | 382 | 380 | 2 | Candidate |
| `particles/cinefx.pcf` | `5addb881a7cdc14dba9f891d88136dac84b11879bd59aa257ba1ec4678e5ec4e` | 38 | 24 | 14 | Candidate |
| `particles/impact_fx.pcf` | `d70ebda940a55bfad173d322754be69245acfd09530ff7e82ab9d8a9538173cb` | 29 | 29 | 0 | Candidate |
| `particles/conc_stars.pcf` | `7045c0f31df34b5ab44e3722678d972977714194fd623704ffb9c74e8743c5d7` | 6 | 6 | 0 | Candidate |
| `particles/class_fx.pcf` | `18faf0845c7eb300a2805b6c1a50fc27a4785c92954f2f233fcfd4e44036c10f` | 99 | 99 | 0 | Candidate |
| `particles/dirty_explode.pcf` | `785ed2cf56114defa02749905ca182334aa8814d5941fcb4cefce61a90e46acb` | 25 | 25 | 0 | Candidate |
| `particles/smoke_blackbillow_hoodoo.pcf` | `de0dcbbb9054355f9107d5c00afc3e8ca5ce58fa1f78f01ef3abee7a6cba9520` | 1 | 1 | 0 | Candidate |
| `particles/scary_ghost.pcf` | `585ac0673b5783219a28173b0d561fa13aeaac1eb8b65daf71dddb8c40382ecf` | 29 | 29 | 0 | Candidate |
| `particles/soldierbuff.pcf` | `8810500db2f8b34eba8352940dfebd61d7c6e9864b6959dfef7c210cfa162a65` | 15 | 15 | 0 | Candidate |
| `particles/level_fx.pcf` | `71e56a2310d2ade170202e95f50a6a435adccd379426df1df2e4c5cd351930b2` | 63 | 63 | 0 | Candidate |
| `particles/training.pcf` | `3381154199cc9b3bae8432a40e68c993c0feba28a55356c77e06e29a445c1a25` | 11 | 11 | 0 | Candidate |
| `particles/stormfront.pcf` | `d8e55983642eaa8a0a0e8551f6752c434be68c589a536cab996c6a7c16e74a43` | 2 | 2 | 0 | Candidate |
| `particles/coin_spin.pcf` | `7f9c9e4e35311495b4f04cfe92c50cc82ba80b7075c89b2e1f0d0e724d6708ca` | 1 | 1 | 0 | Candidate |
| `particles/stamp_spin.pcf` | `b823e59a97e20d28644cfb98b58ec75b98e6be84ca31f6e7dfcabfd5bc1dd4d9` | 1 | 1 | 0 | Candidate |
| `particles/rain_custom.pcf` | `ccca7a0a226a16e9404e2daee730c0856ccb5aa0552ceaed03403359f5e17fa4` | 12 | 12 | 0 | Candidate |
| `particles/npc_fx.pcf` | `3ed407a9f777f216c9001e9683a669d228d6412a01c6388a3221cff128639f36` | 7 | 7 | 0 | Candidate |
| `particles/drg_cowmangler.pcf` | `7b38780bacce1a7e073537e33839e51364892f42e96cb3552a773db6095a2ceb` | 78 | 78 | 0 | Candidate |
| `particles/drg_bison.pcf` | `21e257b2dabc4a12f4ba96d447ab780d034f171dd007663b84fbeef44c47228a` | 29 | 29 | 0 | Candidate |
| `particles/dxhr_fx.pcf` | `e977ddc1ad11d545ef62d5b9c9f7aa41487e17214038010f9a17ff06d07e5538` | 71 | 71 | 0 | Candidate |
| `particles/eyeboss.pcf` | `acfd372abfba758edc74ed5c306586987b1d9aee8d860b08c5819613bc57c4b9` | 68 | 68 | 0 | Candidate |
| `particles/bombinomicon.pcf` | `aefed765034fac44204a385d86eb44e466515890e30c2d608c2380756a643ba3` | 4 | 4 | 0 | Candidate |
| `particles/harbor_fx.pcf` | `1436026abbcf1634e79174e38bbc433126137db01269ebac91838e98cb33fddb` | 11 | 10 | 1 | Candidate |
| `particles/drg_engineer.pcf` | `3ade30706cdf0670f6f4f99cd84b426eca243ebdaf04d01a02d80201e396715b` | 30 | 30 | 0 | Candidate |
| `particles/drg_pyro.pcf` | `3189fa7bc9a76676b5bc80d5964f0da513d11691c512dbf54d64138c5da76bbd` | 63 | 63 | 0 | Candidate |
| `particles/xms.pcf` | `033c79908878d100b02de1e92ff42db912a812e90a3278efa7824c04d5533085` | 22 | 22 | 0 | Candidate |
| `particles/mvm.pcf` | `7ac9e564d2b39a29998c9089c270b954e095f72c3bb1c01410b166efc157316e` | 120 | 120 | 0 | Candidate |
| `particles/doomsday_fx.pcf` | `a16d99440f115844627dc7c9a04d02c3b5ff7e0d8ab14aef80c68f4f58579334` | 11 | 11 | 0 | Candidate |
| `particles/halloween.pcf` | `cc70cd7da704aa517eaea9e4e439da9c780af65b6e5c39be7caf66fd9d112036` | 227 | 227 | 0 | Candidate |
| `particles/items_demo.pcf` | `65f29ffee91f67cef39277391eb6b4cd3a47a810dcd197c28ea01fc2a91791b0` | 16 | 16 | 0 | Candidate |
| `particles/items_engineer.pcf` | `acb8d79600b6a0b92792f552b218994373fb30566c5cace2b6ef60fd42100a94` | 6 | 6 | 0 | Candidate |
| `particles/bl_killtaunt.pcf` | `56881da63683992af26f14f1d9af7ce10986eafd1b4dc14ec099b5013384d854` | 45 | 45 | 0 | Candidate |
| `particles/urban_fx.pcf` | `5feb4be283ace3dfd0cf5e821c1ca4144dfc6ed771128063681a9206a3aa3dae` | 1 | 1 | 0 | Candidate |
| `particles/killstreak.pcf` | `2007fd7f13f12cacf5a3e3db9fa6498de2fcffd0203e325486b8a731b4891041` | 20 | 20 | 0 | Candidate |
| `particles/taunt_fx.pcf` | `10a3118ca584539ec123dfb3a3f20f4ffa97c3cdff144c1de5f61773e170159d` | 288 | 288 | 0 | Candidate |
| `particles/rps.pcf` | `db13d5c4686564e06c27b5abf3915c9e20beb365a187c2316a34db29b6583991` | 45 | 45 | 0 | Candidate |
| `particles/firstperson_weapon_fx.pcf` | `39fcf3ff886260ff43c04574b998f0feca71b3adc0a25c4cd5cd00cbf1888da1` | 5 | 5 | 0 | Candidate |
| `particles/powerups.pcf` | `a351019d2a58e5fd507fe083afdc00ba64d92b3becf090347adf520c8b9c3685` | 52 | 52 | 0 | Candidate |
| `particles/weapon_unusual_isotope.pcf` | `ba52d056fff25702acbdeda55381d1000fb27279af5ccfb9d83e6618b63befe4` | 567 | 556 | 11 | Candidate |
| `particles/weapon_unusual_hot.pcf` | `0ec0e4c80553c22c366f64147aafab7a6b4249d51a0d5a55ca1635a0d4e357d9` | 338 | 327 | 11 | Candidate |
| `particles/weapon_unusual_cool.pcf` | `7263ec435f990960057a8756e90de53e83d404c6b87652b0f365932606a26bc8` | 451 | 440 | 11 | Candidate |
| `particles/weapon_unusual_energyorb.pcf` | `0e376a904ad8d6115b27ca12e01cff0b4c3029b41095836c4e2f875597ffe4ce` | 40 | 40 | 0 | Candidate |
| `particles/passtime.pcf` | `9261d2e87c764fdf2cf9b3194f9a2cd67061f636428b517287dc5bbb4468ea2e` | 1 | 1 | 0 | Candidate |
| `particles/passtime_beam.pcf` | `208ebcc77a8b54f143adbe77ec82ab8766e16315c13a19a8c5564de1866cb79b` | 9 | 9 | 0 | Candidate |
| `particles/passtime_tv_projection.pcf` | `d35c6ec664c8ff1cb3edb5ff086da720beb1dd6378d0bc1f128d2f918363e8c5` | 4 | 4 | 0 | Candidate |
| `particles/vgui_menu_particles.pcf` | `6c6eb86b923b5f872120f2afea8631fdb41540a26e81592ec23ee087c45f94cf` | 13 | 13 | 0 | Candidate |
| `particles/invasion_ray_gun_fx.pcf` | `a4fb4a4d868ebd2470aee668c19cd18a33405a8de33aff0d496c22f93aeaa953` | 32 | 32 | 0 | Candidate |
| `particles/invasion_unusuals.pcf` | `18c01fa1acf7935fe9642b3dbf3765a2588d5d7865bf07c848022cb117e12ae7` | 62 | 62 | 0 | Candidate |
| `particles/halloween2015_unusuals.pcf` | `cbc913a2fc5608828a04c81f94ad4e11ec1514d898d5ff341067b2586900dfb3` | 44 | 44 | 0 | Candidate |
| `particles/rankup.pcf` | `900ae08955fc154c9e135f7850ac442aa96f2f3c351049122289ffa17ed8b9f0` | 16 | 16 | 0 | Candidate |
| `particles/halloween2016_unusuals.pcf` | `712b1ffdcd4cbc01abd1ff82bcb9daa55652a9e8feba894179b43e6492c09d09` | 25 | 25 | 0 | Candidate |
| `particles/rocketpack.pcf` | `cbb355f97f82093d8d84533e58696c12492033f6892a2afb18fdd5122fbb7496` | 6 | 6 | 0 | Candidate |
| `particles/smoke_island_volcano.pcf` | `66fd59c6883b732b6d2ccf930b47add5be2d398afaba856e31ec8d6f9bd755a8` | 2 | 2 | 0 | Candidate |
| `particles/halloween2018_unusuals.pcf` | `9500801398d38da5fbc4d8b89d79dc2c1bb0228f6fe1a3a3defdc00a3f802457` | 136 | 136 | 0 | Candidate |
| `particles/halloween2019_unusuals.pcf` | `abe9a5027f271cb8af3080fa048e53bbc3946f2614db16c93fe8ecd6d16967b6` | 139 | 139 | 0 | Candidate |
| `particles/smissmas2019_unusuals.pcf` | `cede7af0953f6fc7beaab053ad65f9f67a24f125b6f24c9bbcd3ee59aecce561` | 131 | 131 | 0 | Candidate |
| `particles/summer2020_unusuals.pcf` | `90c937dba8cab6eefc12c729bbd9d745a24ab44edc90620098c68b353bd88649` | 57 | 57 | 0 | Candidate |
| `particles/halloween2020_unusuals.pcf` | `048c25cd504d81d4129f04dfd6c23555cd9cde0420bdb57a98547180eede22d9` | 133 | 133 | 0 | Candidate |
| `particles/smissmas2020_unusuals.pcf` | `d6d9abcc5831ee02ea970b1a44ad1753c24a73e0c12de76c2586d9566414f209` | 162 | 162 | 0 | Candidate |
| `particles/summer2021_unusuals.pcf` | `ddc3f321390b6b226523db23ba249d76b4fb39c6921177104b8ea4ec40f48c51` | 105 | 105 | 0 | Candidate |
| `particles/halloween2021_unusuals.pcf` | `e3ecf5e0a075828ed3a8adb17cc093a8bf3745980bc6eb8577538f8be804fb8a` | 209 | 209 | 0 | Candidate |
| `particles/smissmas2021_unusuals.pcf` | `cf39e3aa47db5e7ff8e088e0d50c8358fa3246785b7ec5d2f1b023781b1a734c` | 301 | 301 | 0 | Candidate |
| `particles/summer2022_unusuals.pcf` | `1cd3f2b5032efaca525b9d68f3130caeb8be8446e07b04150e5ee4b8c4bfcf53` | 213 | 213 | 0 | Candidate |
| `particles/halloween2022_unusuals.pcf` | `a5c4fff75c7665acbe8a24108f932cf8bd1f12123ce541cb813b51c0c022035a` | 288 | 288 | 0 | Candidate |
| `particles/smissmas2022_unusuals.pcf` | `d3b5ab0dcc95a72227ceac44051481aa31ba2058bfb0176cd71ef823cbe6ac6f` | 217 | 217 | 0 | Candidate |
| `particles/summer2023_unusuals.pcf` | `c8d82b523d8b4f7990b55127e0d362ddd73a5b2da96411708784aad4dc85a843` | 174 | 174 | 0 | Candidate |
| `particles/halloween2023_unusuals.pcf` | `dc4ffe0a8b08186db0a22ac821c61b5d2f34494bb3126f9052f8e9d9d6d33871` | 335 | 335 | 0 | Candidate |
| `particles/smissmas2023_unusuals.pcf` | `c8e91edc79c35971c99f8a1a8075a94429acfc5e1d6bda5c9c306220f19dc4b0` | 207 | 207 | 0 | Candidate |
| `particles/summer2024_unusuals.pcf` | `372a06865408d3982894f439d3dbd7de009a898a9e4c91e1b15159885614ee8b` | 611 | 611 | 0 | Candidate |
| `particles/halloween2024_unusuals.pcf` | `591a2de2642a28475f60c0e06f2eb106799018d0bedb3da1eab14b4342558634` | 249 | 249 | 0 | Candidate |
| `particles/smissmas2024_unusuals.pcf` | `db6b29ae2eeab91784d4cc67297455d50f9697fa3eae22604af29a5c7ce00000` | 213 | 213 | 0 | Candidate |
| `particles/summer2025_unusuals.pcf` | `c0670a64c88e02a0be28c64591aceac3a13f30bd4c1d5082450aa6d5b08d438e` | 299 | 299 | 0 | Candidate |
| `particles/halloween2025_unusuals.pcf` | `e3c5aed2c778e7a31318efa249c2f135c436b2c52302a27dc64a6203affac9c9` | 203 | 203 | 0 | Candidate |
| `particles/smissmas2025_unusuals.pcf` | `0018fd3fd600584e03bbf800b6a6e2f8ab6556e433b1126cdd8724b470899563` | 161 | 161 | 0 | Candidate |
| `particles/summer2026_unusuals.pcf` | `7e622fb1170386eba9377ecc3b4ca390484ddde36c1b9bf747fba1db189d0ad5` | 173 | 173 | 0 | Candidate |

## Map PAK PCF Source Partitions

The rows below contain the 129 distinct PCF byte sequences selected from active BSP PAKs. `Selections` counts ordered map-manifest occurrences. Equal logical paths with different hashes are different source partitions; equal hashes across multiple maps share one Candidate definition set.

| Representative logical path | PCF SHA-256 | Definitions | Selections | Map profiles |
|---|---|---:|---:|---|
| `particles/abbey_fx.pcf` | `690a6774f74b0c396f50ddac0e24c8ca73980b40f2d63d3ebe926c088eb2914a` | 5 | 1 | `2koth_abbey` |
| `particles/alien2_fx.pcf` | `423c27b8e754fd0b74a2df24ce656d46caeef7b3c543a53cbb424bd11b02f243` | 75 | 1 | `pd_watergate` |
| `particles/alien_aly_fx.pcf` | `5a1211ba48624e6e2d4c2b47059546bad6e6466ac52530af4d365fad36ab06df` | 51 | 1 | `pd_watergate` |
| `particles/alien_crash_fx2.pcf` | `9e5de42cbe0f1df5a90e19fd346633aed25aa909b40670944937f470f49a4664` | 29 | 1 | `koth_probed` |
| `particles/alien_egan_fx.pcf` | `d78fbedaf712b995ed3c7a8b845dd4b632b9fbb7f97cf4957cecf6e985d337e1` | 30 | 1 | `pd_watergate` |
| `particles/alien_egan_fx_dx80.pcf` | `6924ab6a2d328882a040d256847f971ca099b80145257f74f36233bc7788eea8` | 24 | 1 | `pd_watergate` |
| `particles/alien_egan_fx_dx90_slow.pcf` | `f774fde9b027feeca2d645821292afec98bf3caead6566839f455a7a47890fcc` | 26 | 1 | `pd_watergate` |
| `particles/alien_fxxl.pcf` | `a46ef91e24e75336de445bf48db5f4191ef288edf9e70bc38f9e29790409d21e` | 32 | 1 | `pd_watergate` |
| `particles/alox_swamp.pcf` | `b0a7e6ec5a0c195b2b2d4a1d09b497e96aa05788a1ded5ffd44631bac363956a` | 3 | 1 | `htf_marshlands` |
| `particles/aquarius.pcf` | `555f34f901fe20e04e71ca1cae37151b2277e65e40867b853cf01791858fb593` | 45 | 1 | `pl_aquarius` |
| `particles/aquarius_meg_parent.pcf` | `f178df9ec4d003e5aaf00a0d20a687867db9ec6d08d9be0f872671390b2fe3e6` | 6 | 1 | `pl_aquarius` |
| `particles/asylum_particles.pcf` | `825436dc3b244c0705cd6a6aa73c35b4c8b8cff2c6dcd89ff088fe15ce64393e` | 43 | 1 | `zi_sanitarium` |
| `particles/atom_fx.pcf` | `264bbdcbd4abb558dfc755e71de3ba0aa16493f8878e430021f09525036909c0` | 36 | 1 | `pd_atom_smash` |
| `particles/aurora_borealis.pcf` | `adcb66936ee90e66ca21821dc5de750ce4e7f34d2f01abbfc93efbd0be4150c0` | 12 | 2 | `ctf_snowfall_final`, `koth_snowtower` |
| `particles/bagel_event_fx.pcf` | `857ecf7a861e082172600c40f96994c32b08cc33fc0517422006528de70b0d29` | 147 | 1 | `koth_bagel_event` |
| `particles/breadspace.pcf` | `5097b44a6f973e978017aebdea6e580e276e1e1592350a99f17105e77310faad` | 40 | 1 | `pl_breadspace` |
| `particles/brine_salmann_goop.pcf` | `f72e6b65f1a0967bf2a08a971b1d612f0560cd0f06ed7a33733071c6d444933d` | 45 | 1 | `koth_slime` |
| `particles/bugclouds.pcf` | `23054ac514bab75ab7f32677f13d1f8f751714f3ee9064bc4e7562475647bb6b` | 5 | 1 | `cp_darkmarsh` |
| `particles/buildingdamage.pcf` | `2bab9492a78193f05e7ccfa0231bd0ff187bd82133d40cfa92355cb38bcd595b` | 30 | 1 | `koth_toxic` |
| `particles/byreboom.pcf` | `4464adb68cad65d1927e015cf81147eddee6a32281389597bd4b5d183857adc3` | 11 | 1 | `arena_byre` |
| `particles/cachoeria_waterfall_skybox.pcf` | `2a004ba6f1e8758830d38fa597999b46d50d276ec0431978d062865f4658bba3` | 9 | 1 | `koth_cachoeira` |
| `particles/camp_fx.pcf` | `bb3bf7d03a6b36e579edac8bd977910a85f73855d58aeacd8bd7720a5c0c0ef3` | 11 | 2 | `koth_sawmill_event`, `koth_slasher` |
| `particles/canaveral.pcf` | `f90edaf570baf86f58b8e50d43e24c4bf93957af3e3a052259abdfd163f2d54a` | 7 | 1 | `cp_canaveral_5cp` |
| `particles/cauldron_fx.pcf` | `574530ec03e725eebf9be3a77f1bb36aa9c68ef802f90215208bf668435b9520` | 20 | 1 | `pl_rumble_event` |
| `particles/chopper_fx.pcf` | `6256b71586927bcf467bf153e82649b67a0425640878c631fdeec5fe4ded6176` | 2 | 1 | `koth_cachoeira` |
| `particles/coalpit_fx.pcf` | `3a1f264b51ccee26133d872931e1f80e086de37ef39db30528bd7933f17a4173` | 43 | 1 | `cp_gravelpit_snowy` |
| `particles/crasher_fx.pcf` | `7898d9bc2d5629f2a556b0c4508225ffbbaafbf2fd8443fb9e1c88a525d1fd7d` | 95 | 1 | `ctf_crasher` |
| `particles/crasher_fx2.pcf` | `2ca3be7aafd3b57deaa55fea9652804a615e2a4cda6d28d6f40affac02c56fcd` | 4 | 1 | `ctf_crasher` |
| `particles/doublecross_event_fx.pcf` | `88ad6f37317d106960c06305701f0aa8033b2a90b5d543deb4ccd57c0809efbd` | 9 | 1 | `ctf_doublecross_event` |
| `particles/drowned_fx.pcf` | `50cee6757631fe8feb11fd03860690c0c4ee2ea279ad06532c105f483a2f07cb` | 77 | 1 | `pd_cursed_cove_event` |
| `particles/dynamite_fx_ism.pcf` | `1f7f4ed0c751a2c10269b8c0a03ab9cd1903fa16db47ef9a288b6d29ef897f3f` | 7 | 1 | `tow_dynamite` |
| `particles/embargo_particles.pcf` | `1779222bff450594bb58139cc113a8ff1f4187f23dbd1ad2dcbfe29f99549779` | 116 | 1 | `pl_embargo` |
| `particles/env_snow_light_002.pcf` | `5406179229fd022bc0b9bbf3ab680e6787c25d095fdee91721c312d3c6ce6ada` | 1 | 1 | `pl_frostcliff` |
| `particles/env_snow_light_003.pcf` | `b6b818e64f15515912cfe3fc7fee2b0bf418f3cbbcfbf9743bf9a7786a6fb7f1` | 1 | 1 | `pl_frostcliff` |
| `particles/eotl_lights.pcf` | `20f4286cb99d1f865e0e70a3ecae26c7ac5f19c70179400a4aa48df35c64c033` | 6 | 1 | `cp_snowplow` |
| `particles/explosion_high_copy.pcf` | `d6e281a6b7f0ca58657930e530ba44a373ed7ee091827ba2e5b593436ac366f1` | 54 | 2 | `ctf_doublecross_event`, `plr_hacksaw_event` |
| `particles/explosion_snowmann.pcf` | `663f63d377ce4f0103ac6ed33a88f7a2a11ee2e8397e8b672cc2627f4af8a98c` | 13 | 1 | `cp_frostwatch` |
| `particles/falling_leaves.pcf` | `981151b375447798e8e68dacd84b7002c108c969486fcdf4d8d1897380fc5f7b` | 4 | 1 | `pl_rumford_event` |
| `particles/farmageddon_scarecrow_blood.pcf` | `2cf9e45fe74231c1984baac0290d06879fe99e92da9da2463dd8a82fa101bd73` | 4 | 1 | `pd_farmageddon` |
| `particles/farmageddon_underworld_fx.pcf` | `78030dda0e913c82a4b372dad82da4173d92232744cafafd3d3b3e443c2cf606` | 24 | 1 | `pd_farmageddon` |
| `particles/firefly_fx.pcf` | `33146d50d6ee66dfdbde9e878ea960f5df20746745c4907acb2fd19554a9d649` | 4 | 1 | `koth_megalo` |
| `particles/funhouse_effects.pcf` | `1dfef2d51f689cd6197e91e71fca5625294b387e1473653ad84a435a3e330542` | 4 | 2 | `koth_boardwalk`, `koth_slaughter_event` |
| `particles/fx_jumppad.pcf` | `bef7aafd2031d2ed7f0350f9eeca3efd29a315e1faf225edb46d247b4777c858` | 26 | 1 | `plr_hacksaw` |
| `particles/gavle_fireworks.pcf` | `8724c284b26b5325c0ef80105b546dc98deea3bdbce934034e52bae705cb940f` | 26 | 1 | `cppl_gavle` |
| `particles/green_toxic_smoke.pcf` | `0a1d5325ad6c4eca93281d3c1fb718d02a20d53dc079f10bff04e2083f90ab34` | 2 | 1 | `koth_toxic` |
| `particles/haarp_reactor.pcf` | `0029c032f006ee3748741b6761d5ce16cf7daae902c0c85365555939aeee1c63` | 4 | 1 | `ctf_haarp` |
| `particles/hacksaw_event_fx.pcf` | `8a3d92bd3d023d5a3b6178b48947df5791846445810777bf29b8064023b930b1` | 19 | 2 | `ctf_doublecross_event`, `plr_hacksaw_event` |
| `particles/hacksaw_particles.pcf` | `a385b9494f899e602d55d8b3d3488a19d42b3b66a152502c0ed1e495dfcfdb6d` | 6 | 3 | `ctf_doublecross_event`, `plr_hacksaw`, `plr_hacksaw_event` |
| `particles/hadal.pcf` | `d7600816a31bee5ffc2ac2b21abb3cd88e9b8ab8b7b767bbfe3e226f1c914159` | 2 | 1 | `cp_hadal` |
| `particles/halloween.pcf` | `cc70cd7da704aa517eaea9e4e439da9c780af65b6e5c39be7caf66fd9d112036` | 227 | 1 | `pd_circus` |
| `particles/halloween2021_unusuals_copy.pcf` | `e3ecf5e0a075828ed3a8adb17cc093a8bf3745980bc6eb8577538f8be804fb8a` | 209 | 1 | `pd_circus` |
| `particles/hell_of_a_mann.pcf` | `37701b1aac63f3002b5cc9a548f7263f73eea892709d651fe951da2a83353736` | 13 | 1 | `vsh_outburst` |
| `particles/helltrain_brakes.pcf` | `65594c9d4b1cc862b1a4bc75df1091f48d91c204f21b501bd8d99c2365784566` | 2 | 1 | `ctf_helltrain_event` |
| `particles/helltrain_portal.pcf` | `d898ec7020769bc5a6d5de5a65c483a5f968384628da6a887fce3c5baf75200b` | 3 | 1 | `ctf_helltrain_event` |
| `particles/helltrain_rockfall.pcf` | `e53a95c77af0bd94886b2082e2d4358af18c205ac31d7d80680326ba18313024` | 3 | 1 | `ctf_helltrain_event` |
| `particles/helltrain_smoke.pcf` | `e650731c16d3680cbef9bc3cba5ad1ac57fbc0a69c3ddff56a221a6955836bb1` | 3 | 1 | `ctf_helltrain_event` |
| `particles/improved_torch.pcf` | `c8c2c0964d385aafc195d9ba8892c322b3f4166febae235498c61e861ae6900a` | 4 | 1 | `koth_megalo` |
| `particles/infection_particles.pcf` | `8db2646f9cc28052982cb21ba6fe54ea165e40e7641ee289efdfcad873ff77d7` | 47 | 6 | `zi_atoll`, `zi_blazehattan`, `zi_devastation_final1`, `zi_murky`, `zi_sanitarium`, `zi_woods` |
| `particles/invasion_2fort_fx.pcf` | `08ee81b27a3c0cea93c95ec570fec3f862cb8b28d43a559f6c499ee8a9d9c968` | 66 | 1 | `ctf_2fort_invasion` |
| `particles/invasion_2fort_fx.pcf` | `38a655c14acb9d5c5c4ef03dd799e24ad3bfd31fff792159b17155179f516280` | 56 | 1 | `pd_watergate` |
| `particles/item_fx_copy.pcf` | `f6538d1f6f5da9472e59e00dcdfb324b3be006705120da9ad68509f05b8fe07f` | 379 | 1 | `pd_circus` |
| `particles/junglesky_fx.pcf` | `7f45e0bb5bd1cffe2b7e284433fb2a29089bf23d30d4c6ada062865f4658bba3` | 4 | 1 | `cp_fulgur` |
| `particles/koth_dusker.pcf` | `48c32f7d3a9a268880d29902ed6d031cae644163aaedd73dc525d4a7bd436a7e` | 15 | 1 | `koth_dusker` |
| `particles/koth_krampus.pcf` | `4cddc5836cfe70c8993f514481ea65578fbe11d55c9d52cb4747c53dbae14dfd` | 31 | 1 | `koth_krampus` |
| `particles/koth_oilrig.pcf` | `f2327af1f21349b65e5eda2e63ad056c3fac62567a8c59adc3c75d52f60ceb0a` | 19 | 1 | `koth_blowout` |
| `particles/koth_probed_fx.pcf` | `82a8ca15fac8a02b52d81dc38653cb80265e5a77b0b20ac83c202b1f158108d2` | 135 | 1 | `koth_probed` |
| `particles/krampus_portal.pcf` | `c0497ced56bc997600a1eeeb60b5488517f813a4ba140719be05639a11a8fa1d` | 7 | 1 | `koth_krampus` |
| `particles/lab_fx.pcf` | `4aa48544c39962d63d9daa362cb18e117e6a453413379a72fbf22a0aa402fba9` | 63 | 1 | `pd_monster_bash` |
| `particles/laser_effects.pcf` | `d30bb24b11ffef6ca3fd72d99f0c970cef4bd05a97f8cd356dc81bd0c6021843` | 14 | 1 | `cp_snowplow` |
| `particles/laughter_fx.pcf` | `e18f791c6d55b8495d3260fdea2dfc1aca922f4dec9fdd1ce862dbe35e7abac3` | 2 | 2 | `koth_boardwalk`, `koth_slaughter_event` |
| `particles/lumberyard_event.pcf` | `f79a447285342f7f5337ce6d873a6e3afbc06f4e0d61d5a1edb71b8d09c823ba` | 5 | 2 | `arena_lumberyard_event`, `zi_devastation_final1` |
| `particles/lumberyard_event_rarespell.pcf` | `f5fa70bb4ba28353437979c89849847602d41f8439b43bc8aab3c7f3d5b7100b` | 9 | 1 | `arena_lumberyard_event` |
| `particles/mall_rocket.pcf` | `325ef08ed70c4f8cb70ec19f422375dda45faf90df332dbf2240b62185c3509a` | 2 | 1 | `pd_galleria` |
| `particles/map_afterlife.pcf` | `d0c754254900e3ff907e1d62c0b0c0b171f5f43727f857a0d583382a746cdad1` | 21 | 1 | `arena_afterlife` |
| `particles/map_hearth_event.pcf` | `d5aace337ea7b57a9a234fc4cbd9ff903e22b48dc12036935d2f4783b528607d` | 19 | 1 | `ctf_helltrain_event` |
| `particles/matterhorn_beer_explosion.pcf` | `1a0d046a74272e499bec1b9602b62cef604bbdba08f9b380614a7aa83c4bcf8c` | 29 | 1 | `plr_matterhorn` |
| `particles/matterhorn_particles.pcf` | `b9da49021eb00b253d5faf35407e8b1a73641662e55962bcab18b371d5a7cdea` | 108 | 1 | `plr_matterhorn` |
| `particles/matterhorn_spawn_area.pcf` | `e600964c5409e3a7ac67ecae6bb25a0c4d1a8b6d437ac6be02676346b45a1b14` | 42 | 1 | `plr_matterhorn` |
| `particles/megalo_fx.pcf` | `ac2ef7db69aad3af6b55c7b0eaddfd5325bbe7c898a844ef6796078ca33a0779` | 21 | 1 | `koth_megalo` |
| `particles/megaton_particles2.pcf` | `f400884e8e1904272c3e7da1917354bc495169cdb643a2282251cd79b969804a` | 10 | 1 | `koth_megaton` |
| `particles/modular_weather_fx.pcf` | `61921e14a638c23d294b12cf2999c6bf57ec15dfefa7dfba3127e5b50de4347d` | 42 | 1 | `ctf_snowfall_final` |
| `particles/moldergrove_hwn_fx.pcf` | `328c37ccda30603cf5a04e4d7a5b4ec8517b7baaba1ed33bea4689913b78a6a3` | 14 | 1 | `koth_undergrove_event` |
| `particles/muertos_petals.pcf` | `3984c9ad9adade79be633e9c5ee24dc0e659fd079d501920fb6e823852a9d401` | 2 | 1 | `koth_los_muertos` |
| `particles/nucleus_event_effects.pcf` | `030d8f2a4f4b7653c4fcd76d8fbad51a66b99ce39ffb6b115fcb32ac83922af6` | 65 | 1 | `arena_lumberyard_event` |
| `particles/nucleus_event_effects.pcf` | `40fd45f11f64f2ec0b7c544a2119ab90acdeb25254834b783d502b415368e488` | 11 | 1 | `pd_circus` |
| `particles/papertrail_pressure.pcf` | `d7b9b095de569dc33a2e93c3a3bb11bae231dcc64ec6a17fdf4b45018949e531` | 1 | 1 | `ctf_pressure` |
| `particles/particles_vsh_abilities.pcf` | `43c956314c3140b4acb52e542b9dab4359b9a9f1e82e9ab623d7d6af9f8908b9` | 15 | 1 | `vsh_maul` |
| `particles/particles_vsh_abilities.pcf` | `9029b6cf2a4fe70086afe831cf8e2dcf415dff9775c903051056f91cac790205` | 14 | 1 | `vsh_outburst` |
| `particles/particles_vsh_abilities.pcf` | `d95a346a6cb760dbd2897f65267e8061890da766dabacbf54ba41a4973186fa1` | 12 | 4 | `vsh_distillery`, `vsh_nucleus`, `vsh_skirmish`, `vsh_tinyrock` |
| `particles/particles_vsh_auras.pcf` | `369f831fd39cb06f7c2833e43068e16715bb5bfed0d4432f7e7b7043f8e1c455` | 52 | 1 | `vsh_maul` |
| `particles/particles_vsh_auras.pcf` | `379a10920ccefdaeea5d6cc67461647c7f8327229f493371bb9f9c1b86bbe886` | 47 | 4 | `vsh_distillery`, `vsh_nucleus`, `vsh_skirmish`, `vsh_tinyrock` |
| `particles/particles_vsh_auras.pcf` | `f28414f81f8d93b48eb67a57196ac5e4a8eb12168b8127115ea007b0241e1be9` | 50 | 1 | `vsh_outburst` |
| `particles/particles_vsh_overrides.pcf` | `0a6007d1850ec16559e3ff7b8b22e4dd10c740c336873276d88ac9b21259abfb` | 53 | 1 | `vsh_maul` |
| `particles/particles_vsh_overrides.pcf` | `3dce06e05388426d39efbb0f2dfb61398106a12af58d49e24fe107adfa920e66` | 53 | 4 | `vsh_distillery`, `vsh_nucleus`, `vsh_skirmish`, `vsh_tinyrock` |
| `particles/particles_vsh_overrides.pcf` | `69990622910326a291fc886c5b8947401d6850819ac15b9b040676405d40263f` | 53 | 1 | `vsh_outburst` |
| `particles/patagonia_particles.pcf` | `f7f2b5d1f1098b0d676a7df84e9e0f035c35e2b0f10bec5beb731ff0de098cf8` | 4 | 1 | `pl_patagonia` |
| `particles/pd_icons.pcf` | `6258143074819b08285d821d49dd734b12355c5572f1bfd5361cf0a51931f065` | 106 | 1 | `pd_watergate` |
| `particles/phoenix_particles.pcf` | `60dd1a234dfb9c0afbf7a7e7a0c66f13fdb263a829124d824320f87c79314995` | 3 | 1 | `pl_phoenix` |
| `particles/pier_fx.pcf` | `3bf53f154bab6806ba61ea3d592aac6d40a696c5d615bdce3f36cc9cc18bfd8b` | 69 | 1 | `pl_pier` |
| `particles/piranha_fx.pcf` | `dce0e554c19de657f14cf60f6336dc0505f78a8cb7740aab7968fe769409c62f` | 9 | 1 | `koth_cachoeira` |
| `particles/pl_snowycoast_fx.pcf` | `048cac124de62578f0caa28bc739922b694502d6a94a99bdc18448760e456fc7` | 44 | 1 | `pl_snowycoast` |
| `particles/playerdestruction.pcf` | `a2dffa06c14dec737b14e8b5d6342aa9da5c9cd93524943a03acefb80d98923d` | 6 | 1 | `pd_watergate` |
| `particles/polar_aurora_light.pcf` | `fa6b23a91ed43ab6d10a413fef17e9826e32d6afad9127fd61253aa8dfd69c39` | 2 | 1 | `pl_coal_event` |
| `particles/potion_fx.pcf` | `cf1c5165716f03b2468e561e8a0218559ebbf707e6c949ec318d3372fdb994dd` | 7 | 1 | `pl_rumble_event` |
| `particles/precipice_spooky_fx.pcf` | `83bc019baeaca5478dbddc8900e8b207d7708c4d495d36ee8f84ba8f07b0e369` | 129 | 1 | `pl_precipice_event_final` |
| `particles/pressure_particles.pcf` | `dd1ba4fb97b9df8cd02c634d8024318e6a89196b46d4c69e43448b99f15110f1` | 23 | 1 | `ctf_pressure` |
| `particles/pyre_fx.pcf` | `0bca42e0676e01ddbc7cc32ce0b81e074418a250c994930b6655fc02e907631b` | 10 | 1 | `zi_blazehattan` |
| `particles/pyre_fx.pcf` | `38ab1e001905a71410e277e715c0ee6c7a2462e894d9b3d7b105fd4a663132fc` | 7 | 3 | `ctf_doublecross_event`, `plr_hacksaw_event`, `zi_devastation_final1` |
| `particles/rain_premuda.pcf` | `d5269816b583c96ad3547b147fab370d6e4de613d5f7a1b9a401ae9b2c20384a` | 12 | 1 | `cp_premuda` |
| `particles/rain_with_collision.pcf` | `5efe459ca3538eaf4b843262caa8bfd80dcdf095c3b6b97779c480502fcd48e1` | 8 | 1 | `arena_lumberyard_event` |
| `particles/rats_fx.pcf` | `d12553944708dd46019a14aad7732438fcba8e1212a7eb333a4b72dd293db4bb` | 29 | 1 | `cp_degrootkeep_rats` |
| `particles/scary_ghost.pcf` | `ccbbe8d3b5e87e64c42d00f6157dec28474667775a46546acf837cdc49906d87` | 28 | 1 | `plr_hacksaw_event` |
| `particles/seaghoul.pcf` | `8a56bd57ded25074512367c755699f234514fde61dc56474b7777f0b0773931b` | 16 | 1 | `koth_slime` |
| `particles/seagull.pcf` | `f87f4f2bf724d3aa4b9a80d67d63331a08c3ce18e1a20d0aec23cae37edcc37f` | 19 | 1 | `koth_boardwalk` |
| `particles/sewer_env_fx.pcf` | `259170b5f2a662665f6d57a99fb2dab23faa016e9bda039aa7ae7e35aec4e384` | 19 | 1 | `koth_mannhole` |
| `particles/slaughter_fx.pcf` | `0f5cbf19e0f1ed7dc326a6fe34257e48838ade7a74e21d5e16ed7d99e129b961` | 15 | 3 | `pd_atom_smash`, `pl_rumford_event`, `vsh_nucleus` |
| `particles/smoke_blackbillow_eotl.pcf` | `159cdb05417dad13a51a3750f320bd0d52c5b3b6edad692c195f896bdca50a6c` | 2 | 1 | `cp_snowplow` |
| `particles/snowplow_snow.pcf` | `0b8256371de4e9b277a9b96c678dd2b30828784ee33dd9e47ccc7813076528eb` | 12 | 2 | `cp_snowplow` |
| `particles/snowplow_sparks.pcf` | `da5a7f3f4dc442e1478a13b0ff81ed2256b070dbee233557d9dee748c6bc5ead` | 1 | 1 | `cp_snowplow` |
| `particles/snowplow_train_explosions.pcf` | `42178d7f1dc4f67ee96409332931c8bd283b62dbc32efb0c205364c34113d038` | 6 | 1 | `cp_snowplow` |
| `particles/snowville_particles.pcf` | `85436aa8c510b63558f15372fa990e3f1e9319b00781f2b9b82a3c50c1fb130c` | 11 | 1 | `pd_snowville_event` |
| `particles/soul_core_red.pcf` | `aefab5746ad783cfa5ce340fda3e241abcd0053cb3fffb7f38733e1069da3064` | 2 | 1 | `pl_corruption` |
| `particles/sparks_copy.pcf` | `56b1c41a6049a4bbd2e235917f0ce8b29aa677118f90176f1f5b2ec31b4dbaa2` | 22 | 2 | `cppl_gavle`, `plr_hacksaw_event` |
| `particles/stone_fx.pcf` | `41d02077926493742ba8cb93bf08d690a15915920dd265a0db7ea4be9351c5f5` | 15 | 1 | `pl_rumble_event` |
| `particles/toxic.pcf` | `f53eb4081ab90b02bc0759c6ad9e0ad6975abfd708ae0dd18ee312f2d97bc459` | 2 | 1 | `koth_toxic` |
| `particles/vineyard_rain.pcf` | `b195fbd7c7c83c560dc6f1488c7b9e43826fdaeba668f6d5a5f0c7208d5a5732` | 7 | 3 | `koth_toxic`, `pl_corruption`, `zi_sanitarium` |
| `particles/vsh_maul_particles.pcf` | `b2553f45de35b8704744ca5fa85709e78220d1cf5be244e6208336cd11d0c8f4` | 20 | 1 | `vsh_maul` |
| `particles/water_copy.pcf` | `fe0b8f359c6fefcda9b664dc8ef0f2cecc75326823e2746832e986bba377625d` | 42 | 2 | `cp_brew`, `plr_hacksaw` |
| `particles/waterdrips.pcf` | `c8861ad9fe183a0f0b33da3aadb95de8636d4e44012b81adbf2302d57d892b65` | 2 | 1 | `arena_lumberyard_event` |

## Generation Contract

The future generator must resolve the accepted provider plan; parse only exact indexed global and map manifests; read only manifest-selected PCFs; verify every VPK, BSP PAK, manifest, and PCF hash; emit one stable item per `(PCF SHA-256, definition UUID)`; retain name/UUID mode, source name, manifest order, provider, map profile, shadow state, all fields, child edges, operator occurrences, references, maxima, and coverage classification; and fail on a missing provider, unclassified entry, duplicate stable ID, malformed selected document, stale hash, unresolved child, changed profile result, or item-count mismatch. Missing, Unknown, Unsupported, Malformed, and always-shadowed items remain in output rather than being omitted. Two clean-work-directory generations must be byte-identical.

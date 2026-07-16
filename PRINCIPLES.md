# Principles

- Target observable Source 1 parity, not visual plausibility.
- TF2 is the first complete parity target.
- Source 2 is out of scope.
- Organize by Source domain, not programming language.
- Make every package independently useful.
- Prefer compiled data over reconstruction.
- Maintain one current implementation and contract.
- Prefer breaking changes while there are no external consumers.
- Remove replaced code, fallbacks, legacy paths, and duplicate authorities.
- Preserve handled, inert, unsupported, malformed, missing, and unknown states.
- Keep gameplay, replay, and rendering authorities separate.
- Treat performance, memory, backpressure, and repeatability as correctness concerns.
- Use content addressing to avoid duplicate immutable assets.
- Keep apps lightweight: configure and assemble packages.
- Use repeatable scripts instead of command-line argument recipes.
- Resolve content through exact configured roots and logical paths; never scan a machine.
- Add tests where they provide fair and useful evidence; do not create test theater.
- Track parity through explicit `Source/TF2 vs playsrc` comparison tables.

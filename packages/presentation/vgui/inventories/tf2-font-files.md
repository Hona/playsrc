# Configured TF2 Font Files

Owning roadmap: [`../ROADMAP.md`](../ROADMAP.md).

## Fixed Content Identity

The configured TF2 public build is `24245096`, patch `10828683`. These eight loose files are game-supplied UI font sources. They are not substitutes for the platform-local Tahoma, Lucida Console, Verdana, Helvetica, Monaco, Courier New, Arial, Trebuchet MS, or Apple Symbols family requests selected by Source schemes and surfaces.

| Logical identity | Bytes | SHA-256 | SFNT family | SFNT 16.16 version |
|---|---:|---|---|---:|
| `resource/ocra.ttf` | 24,316 | `a0f58809705d54108fe41409bae70fbb8315a64e989aaf2afa04d5cfbb94f54e` | `OCRA` | 65,536 |
| `resource/tf.ttf` | 115,928 | `28b541e28c882b3e732fefe86320e8d6d9e13470b0ed1f26e55b5ca413ccc687` | `Team Fortress`, `Counter-Strike` | 262,144 |
| `resource/tf2.ttf` | 68,828 | `1c36e9e8f8e305fb0a889889bf55a06d0ab9aba13f88d5188ddf87122d5c1af1` | `TF2` | 65,536 |
| `resource/tf2build.ttf` | 61,696 | `23faa58a08c929c0b6638f581488e49399cd7a390c70cb9debdaf8371a95e0c6` | `TF2 Build` | 65,536 |
| `resource/tf2professor.ttf` | 47,480 | `5d6ac5202d90e3b72c5221a96925cab78a641be097f220b7b5aad4f5d26680cd` | `TF2 Professor` | 131,662 |
| `resource/tf2secondary.ttf` | 73,900 | `39c2b96bc4af8c6b7026b60afca816704aba6d6a29c43749bfbac63f8fb7d746` | `TF2 Secondary` | 131,072 |
| `resource/tfd.ttf` | 44,180 | `6507380ac9720feabe55772c90f19898d3efa02b22ab0d3d823e9e94a0b9c27e` | `tfd`, `cs` | 262,144 |
| `resource/tflogo.ttf` | 8,420 | `4a39b596e81426cb718c694685e79b395b1c7d4779100c384428c30ac32f1504` | `Team Fortress Logo` | 65,536 |

Each content-font request must match logical identity, byte length, SHA-256, version, and selected family before browser publication. VGUI retains bytes only for the bounded load operation and never commits or republishes the source file.

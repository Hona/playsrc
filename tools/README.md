# Tools

Developer and operator programs live here. Tools consume public module interfaces and must not become alternate implementations of Source behavior.

## Organization

| Tool | Objective |
|---|---|
| [`playsrc/`](playsrc/) | Run repeatable local, build, verification, and release commands. |
| [`inspector/`](inspector/) | Interactively inspect Source and playsrc state. |

Imported reusable behavior belongs in `packages`; continuously deployed programs belong in `apps`.

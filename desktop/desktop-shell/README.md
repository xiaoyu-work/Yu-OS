# Yu OS desktop shell

The source of Yu OS's Quickshell-based desktop interface. It contains the
Caelestia shell, matching CLI, Material shape library and pinned Quickshell
engine, not an Omarchy distribution or a replacement Agent.

| Directory | Source |
| --- | --- |
| `native/caelestia/` | Complete QML shell, native C++ plugin, resources and upstream build files |
| `native/caelestia-cli/` | Matching Caelestia command-line implementation |
| `native/m3shapes/` | Matching Qt Quick Material shape library |
| `native/quickshell/` | Complete, pinned Quickshell engine |

[`upstream.lock.json`](upstream.lock.json) records the immutable source
revisions and Git trees. The CLI, shape library and Quickshell revision match
the shell's original `flake.lock`. Original licenses and notices remain in
each source tree; see [provenance](PROVENANCE.md).

`yuos-desktop` builds the Shell and its private Qt runtime from this
directory. Root verifies the installed tree against the signed OS release
before launching the Shell in the common GUI Host. There is no App package
or separately signed Shell release. See the [component guide](MODULE.md)
and [desktop build instructions](../../docs/building-desktop.md).

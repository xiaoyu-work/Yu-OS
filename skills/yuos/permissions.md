# Permission Tiers and App Capabilities

Two layers of permissions in Yu OS:

1. **Role / scope on spawned processes** — set when the agent calls the `yu_proc` tool's `spawn` command.
2. **Per-app capability gating** — apps declare verbs in their manifest and the runtime checks them through the hidden kernel policy bridge. App authors do not call the bridge themselves; the `yuos_runtime.policy` helper does.

## Roles passed to `yu_proc spawn`

| Role | Allowed Operations |
|------|--------------------|
| `observer`   | Read only |
| `worker`     | Read, write inside scope |
| `curator`    | Read, write, delete |
| `connector`  | Read, write, network |
| `automator`  | Read, write, exec |
| `agent-host` | Read, write, exec, network |
| `admin`      | All operations |

```json
{ "command": "spawn",
  "args": ["--session", "reader-1", "--role", "observer",
           "--", "analyze.py"] }
{ "command": "spawn",
  "args": ["--session", "builder-1", "--role", "worker",
           "--scope-path", "/home/cos/project",
           "--", "build.py"] }
```

Child processes cannot escalate beyond the parent's role or widen the parent's scope.

## App capability check

Apps written in Python use the `yuos_runtime.policy` helper. The verb
(e.g. `fs.read`, `net.http`) must be declared in the app's manifest. Agents and
end users do not invoke a permission CLI directly; permission requests are
mediated by the Agent UX.

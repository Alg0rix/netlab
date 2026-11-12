# CLI exports of the WASM module

The `NetLab` class in `wasm/src/lib.rs` exposes the [IOS-style CLI engine](cli.md) to JavaScript. This page describes the exports, their JSON shapes, and the session lifecycle. Use it when you write the code that connects a console widget to a simulated device.

## Sessions

`NetLab` keeps one CLI session for each device. A session exists exactly while its device exists.

| Call | Effect on sessions |
|---|---|
| `add_device(...)` | Creates a session at user exec. The session has no interface names and no startup text. |
| `remove_device(id)` | Frees the session of that device. |
| `restore()` | Replaces the topology with an empty one. Frees every session. |

Every `cli_*` export, `baseline_commit`, and `reset_config` takes a device id. If the device has no session, the call fails. On `wasm32` the error value is the string `netlab: device <id> was deleted`. Treat it as "this console is stale" and close the console.

Two inputs are programming errors. They throw a JavaScript exception instead of returning an error value:

- `cli_key` with an operation other than `"ctrlc"` or `"ctrlz"`.
- `cli_set_ifnames` with a name that is not in short canonical form, for example `GigabitEthernet0/0`. Use `g0/0`, `fa0/1`, and similar names.

## Exports

| Export | Returns | Purpose |
|---|---|---|
| `cli_exec(dev, line)` | exec result | Run one typed line. |
| `cli_key(dev, op)` | key result | Send `"ctrlc"` or `"ctrlz"`. |
| `cli_help(dev, beforeCursor)` | help result | Answer `?` for the text before the cursor. |
| `cli_complete(dev, beforeCursor)` | complete result | Answer Tab for the text before the cursor. |
| `cli_prompt(dev)` | string | Return the current prompt. |
| `cli_script(dev, text)` | script result | Run a pasted block or a startup script. |
| `cli_set_ifnames(dev, names)` | nothing | Register the device's interface names, in port order. |
| `cli_set_startup(dev, text)` | nothing | Set the saved startup text. Pass `null` for "nothing saved". |
| `baseline_commit(dev)` | nothing | Snapshot the device config as the authored baseline. Call it after the lab document has applied one node. |
| `apply_device_fields(dev, fields)` | nothing | Apply the codec seat object the lab document carries. See [apply_device_fields](#apply_device_fields). |
| `reset_config(dev)` | nothing | Replay target for a reload. See [reset_config](#reset_config). |
| `ping(dev, ip)` | boolean | Arm a ping job. Call it for the `"ping"` intent. |
| `traceroute(dev, ip)` | boolean | Arm a traceroute job. Call it for the `"trace"` intent. |

Call `cli_set_ifnames` before the first command that names an interface. Pass the device's port names in declared order.

## Result shapes

All results are plain JSON objects. The `prompt` field is the prompt to draw next.

### Exec result

```json
{ "prompt": "R1#", "out": [ ...ops ], "mutated": false, "pending": { "prompt": "Proceed with reload? [confirm] " } }
```

- `mutated` is `true` when the line changed the device configuration.
- `pending` is present only while a confirmation dialog waits for an answer. Its `prompt` equals the top-level `prompt`. The dialog prompt carries its own trailing space. Draw it as is.
- While a dialog is open, send the answer through `cli_exec`. The dialog consumes the line.

### Key result

```json
{ "prompt": "R1#", "out": [ ...ops ] }
```

A key result has no `mutated` and no `pending`. Ctrl+C closes an open dialog. Ctrl+Z jumps to privileged exec.

### Help result

The `t` field selects the shape.

| `t` | Fields | Meaning |
|---|---|---|
| `"rows"` | `rows: [{ word, help? }]`, `prompt` | Keywords that can come next. `help` is absent when the keyword has no description. |
| `"inline"` | `inline: [string]`, `prompt` | The line is complete and nothing else can follow. `inline` holds `<cr>`. Show it inline. |
| `"err"` | `index`, `prompt` | The text before the cursor is invalid. `index` is the byte offset where the first rejected word starts. |

### Complete result

| `t` | Fields | Meaning |
|---|---|---|
| `"line"` | `line` | Replace the input with this text. |
| `"list"` | `list: [string]` | Several candidates match. Show them. |
| `"err"` | `index` | The text is invalid. `index` is the byte offset where the first rejected word starts. |

A complete result has no `prompt`. Call `cli_prompt` if you need it.

### Script result

```json
{ "out": [ ...ops ], "mutated": true, "errors": [ { "lineno": 3, "msg": "..." } ] }
```

- A script result has no `prompt`. Call `cli_prompt` after the script.
- `out` contains `echo` ops. The engine echoes each script line with its prompt before the output of that line. Exec and key results never contain `echo` ops.
- `lineno` counts lines in the text you passed. The rest of the script still runs after an error.

### Ops

Every entry in `out` has a `t` field.

| `t` | Fields | Draw as |
|---|---|---|
| `"text"` | `cls`, `text` | One or more lines. `cls` is `null`, `"dim"`, `"err"`, `"ok"`, `"warn"`, or `"echo"`. `text` can contain `\n`. |
| `"echo"` | `text` | A script line with its prompt. |
| `"clear"` | none | Clear the screen. |
| `"bell"` | none | Ring the terminal bell. |
| `"intent"` | `kind`, ... | A request for the host application. See below. |

The engine does not run intents. The host application does. After it handles the intent, it feeds the result back.

| `kind` | Extra fields | Host action |
|---|---|---|
| `"ping"` | `target` | Call `ping(dev, target)`. See [Probes](#probes). |
| `"trace"` | `target` | Call `traceroute(dev, target)`. See [Probes](#probes). |
| `"rename"` | `name` | Rename the device. |
| `"nvram-write"` | `text` | Store the text as the saved startup configuration. |
| `"nvram-erase"` | none | Erase the saved startup configuration. |
| `"reload"` | none | Reload the device. Read the power state, call `reset_config`, restore the power state with `set_power`, mirror the saved text with `cli_set_startup`, then replay it with `cli_script` when the node's Apply-on-start flag is on. |

The session already updates its own startup text on `write memory` and `write erase`. The `nvram-*` intents tell the host application to persist the same change.

## Probes

`ping(dev, ip)` and `traceroute(dev, ip)` arm one probe job on the device. Both return `true` when the job starts. They return `false` when the device does not exist, the device is powered off, or the address is not a valid IPv4 address. A `false` result is not an error value. Print your own message, such as `could not send probe`.

The probe replies arrive as simulation events. Read them with `drain_events()` after each `tick()`. Events of kind `ping` and `trace` carry the reply text for the console.

`probe_ttl(dev, ip, ttl)` sends one silent ICMP echo with a chosen TTL and emits no probe events. The `"trace"` intent does not need it. Use it to build your own probe loop.

## Simulation events

`cli_exec` and `cli_script` put simulation events in the topology event queue. Read them with `drain_events()`. `cli_key` discards events.

## Flow signals

`drain_flow()` returns one `{tick, link, from, proto}` record per link enqueue since the last call. It feeds the canvas packet trails. A realtime transit is shorter than the snapshot cadence, so `l.frames` sampling alone misses most hops. Flow signals are not console output; do not parse them for wording.

## apply_device_fields

`apply_device_fields(dev, fields)` applies one node's lab-document seats in a single call. `fields` is a JSON object. `web/labyaml.js` builds it from the decoded node; call this export inside the decode loop, before `baseline_commit(dev)`.

```json
{
  "ports": [{ "index": 0, "admin_up": true, "method": "manual", "stats": { "rx": 42 } }],
  "routes": [{ "prefix": "10.9.0.0", "mask": "255.255.255.0", "next_hop": "10.0.0.2", "out_port": 0, "ad": 1, "metric": 0, "learned_tick": 0 }],
  "mac": [{ "mac": "aabb.ccdd.eeff", "port": 2, "vlan": 10, "created_tick": 4, "etype": "static" }],
  "arp": [{ "ip": "10.0.0.9", "mac": "1122.3344.5566", "port": 0, "created_tick": 5, "etype": "static" }],
  "vlans": { "10": { "name": "users", "state": "suspend" } },
  "toggles": { "directed_broadcast": true },
  "ospf": { "pid": 7, "router_id": "9.9.9.9", "networks": [{ "net": "10.0.0.0", "wild": "0.0.0.255", "area": 2 }] }
}
```

- Every field is optional. An unknown key is an error, not a silent skip.
- `routes` carries static routes only. `ad`, `metric`, and `learned_tick` keep the authored values.
- Seat application is config, not runtime effect: `admin_up`, `method`, and `stats` write fields directly and emit no link events. Routes, vlans, toggles, and OSPF ride the same kind-gated setters the CLI uses, so a wrong-kind seat skips. A host keeps its authored default route.
- `ospf.router_id` may be `null`: the router then derives its ID. A string marks the ID as explicit.
- `toggles.routing_enabled` is a constructor capability. The call never rewrites it.
- A malformed `fields` string is a programming error and returns `netlab: bad device fields: <why>`. So does a seat value the setter refuses, such as a port index outside the device, a non-contiguous route mask, or a non-static `proto`. A rejected call applies nothing.
- Learned seats (`mac`, `arp`) apply at open, but a later `reset_config` still clears them, the same as a reload.

## reset_config

`reset_config(dev)` restores the device to its authored baseline — the snapshot `baseline_commit(dev)` last took — and returns the session to user exec. It does these things, in this order:

1. Restores every config field from the baseline: hostname, descriptions, interface addresses and admin state, VLANs, static routes, access lists and `ip access-group` bindings, NAT roles and static NAT entries, DHCP service and excluded addresses, OSPF settings, toggles.
2. Clears learned state a reload drops: ARP cache, MAC table, flow table, NAT translations, OSPF adjacency state, pending frames, probe jobs.
3. Returns the session to user exec. It drops any open dialog and resets the mode context.

The restore also reinstates the authored power flag. To keep a running device running, read the power state before the call and pass it to `set_power` afterwards.

Carrier bindings are topology, not config: each port keeps its link attachment and live carrier state. Canvas position is not config either. Connected routes re-derive from the restored interfaces.

The baseline lives exactly while its device lives:

- `add_device` seeds it, so a node created at runtime resets to factory defaults.
- `baseline_commit` rewrites it. Call `baseline_commit(id)` once the lab document has applied one node's config, inside the decode loop.
- `remove_device` and `restore()` drop it.

It leaves these session seats alone:

- Command history and the interface names.
- The startup text. `cli_set_startup` owns it.

To finish a reload, call `cli_script` with the saved startup text after `reset_config`.

# CLI transcript corpus

This page is for developers who write tests. It describes the transcript cases in `core/tests/cli/corpus/`. Each case replays typed input against the [CLI engine](cli.md) and compares the output with a saved transcript. The cases are test data. They are not console features.

```text
cargo test -p netlab-core --test cli                       → compare every case with its .golden
NETLAB_GOLDEN=write cargo test -p netlab-core --test cli   → rewrite every .golden and web/_test/corpus-steps.json
```

## Files

Each case is a pair of files with the same stem:

| File | Owner | Content |
|---|---|---|
| `<id>.raw` | You | The case: a header, then the input steps. |
| `<id>.golden` | The test run | The engine's output for the case. Never edit it by hand. |

The runner is `core/tests/cli/runner.rs`. Checks that do not use cases live in `core/tests/cli/properties.rs`.

## The `.raw` format

A `.raw` file has two parts: the header and the steps. A blank line ends the header.

### Header

The first line is `## netlab-corpus v1`. Then come `## <field>: <value>` lines. A field can appear once, except `note`.

| Field | Required | Value |
|---|---|---|
| `id` | yes | Must equal the file stem. `c15-startup-mirror.raw` has `## id: c15-startup-mirror`. |
| `kind` | yes | `router`, `switch`, or `host`. The session runs on the first device of this kind in the topology. |
| `provenance` | yes | `authored`, `regression:<ref>`, or `oracle:<ref>`. Use `regression` for a case that pins a fixed bug. Use `oracle` for a transcript copied from real equipment. `<ref>` says where it came from. |
| `topo` | no | A fixture topology. Default `single-router`. See the table below. |
| `tick` | no | Simulation tick at the start. Unsigned integer. Default `0`. |
| `ifmap` | no | Comma-separated interface names in port order, such as `g0/0,g0/1`. Default: the port names of the session device. |
| `masks` | no | Comma-separated mask names. See [Masks](#masks). |
| `note` | no | Free text. Repeat the field for more lines. |
| `startup:` | no | The saved startup text. See below. |

The runner rejects unknown fields, duplicate fields, and a missing required field.

#### `ifmap` names

Write each name in the short canonical form: `g0/0` for a router, `fa0/1` for a switch. The runner rejects long names such as `GigabitEthernet0/0`. The names bind by position. The first name is port 0, the second is port 1, and so on.

#### `## startup:` blocks

`## startup:` has no value. It sets the saved startup text that `show startup-config` prints. The lines that follow carry the text. Each stored line starts with one extra space. The runner removes that space and joins the lines with newlines. A line with no leading space ends the block.

```text
## startup:
 hostname Saved
 interface GigabitEthernet0/0
  ip address 10.5.5.5 255.255.255.0
```

This stores three lines. The third line keeps its own two-space indent. The block must hold at least one line. Use the block only in the header.

#### Topologies

| `topo` | Devices |
|---|---|
| `single-router` | `R` |
| `single-switch` | `SW` |
| `two-router-link` | `R1` and `R2`, linked port 0 to port 0. `R2` has `10.0.0.2/24` and the port is up. |
| `switch-hosts` | `SW`, `PC1`, `PC2`. `PC1` and `PC2` connect to ports 0 and 1. VLAN 10 is named `blue`. Port index 2 (`fa0/3`) is an access port in VLAN 10. |
| `ospf-pair` | `R1` and `R2`, linked. Addresses `10.0.0.1/24` and `10.0.0.2/24`. Both run OSPF process 1 on `10.0.0.0 0.0.0.255` in area 0. `R1` has a static route to `10.9.0.0/24` through `10.0.0.2`. |
| `nat-dhcp` | `R` and `PC`. `g0/0` is `192.168.1.1/24`, NAT inside. `g0/1` is `203.0.113.1/24`, NAT outside. `service dhcp` is on and `192.168.1.1` is excluded. |

Add a topology in `core/tests/cli/fixtures.rs`. The fixtures drain their setup events before the case starts.

### Steps

After the header, each line is one step.

| Line | Step |
|---|---|
| Any plain line | Sent to the session as one typed line. Dialog answers use this form too: after `reload`, the next line `y` answers the dialog. |
| Blank line, or a line starting with a single `#` | Skipped. |
| `## script` ... `## endscript` | The lines between the markers go to `cli.script` as one block, unchanged. Blocks cannot nest. |
| `## key: ? <text>` | Help (`?`) for the text before the cursor. |
| `## key: tab <text>` | Tab completion for the text before the cursor. |
| `## key: ctrlc`, `## key: ctrlz` | A control key. |
| `## tick: +<n>` | Advance the simulation by `n` ticks, then record the events that happened. |
| `## drain` | Record the events that waited since the last record. |
| `## allow-events` | Allow events that no step records. See below. |
| `## gap: <tag> [tok<n>]` | Mark the next line as a deferred command. See [Gap markers](#gap-markers). |

A case fails if events are still waiting at the end. Add a `## drain` or a `## tick:` step that records them. If the case does not care about them, add `## allow-events`.

A case must hold at least one step. Unknown `##` lines fail the case, so a typo does not pass silently.

## The `.golden` file

```text
## netlab-golden v1
## id: c15-startup-mirror
## src-sha: a752d49d2b471375
## provenance: authored
## generated-by: NETLAB_GOLDEN=write cargo test
## do-not-edit

R> enable
R# show startup-config
...
```

After the blank line comes the transcript. Each typed line appears with its prompt. Engine output follows it. The runner prefixes lines that are not plain text:

| Prefix | Meaning |
|---|---|
| `!ok`, `!err`, `!warn`, `!dim`, `!echo` | Output line with that class. Plain output has no prefix. |
| `!clear`, `!bell` | Screen clear, terminal bell. |
| `!intent <kind> ...` | A request for the host application: `ping`, `trace`, `rename`, `nvram-write`, `nvram-erase`, `reload`. `nvram-write` shows the stored line count, then the lines. |
| `!prompt <text>`, `!pending "<text>"` | The prompt after a help, completion, or key step. `!pending` means a dialog is open. |
| `!rows <n>`, `!inline`, `!line`, `!list`, `!caret <n>` | Help and completion results. |
| `!event <kind> <message>` | A simulation event that a `## tick:` or `## drain` step recorded. |
| `!error <line> <message>` | An error that `## script` collected. |

The last line is the prompt at the end of the case.

### Staleness

`src-sha` is a 64-bit FNV-1a hash of the `.raw` file. Before hashing, the runner removes carriage returns and trailing spaces from every line. Any edit to the `.raw` file, including a `note` line, makes the `.golden` stale. The test then fails with `stale golden`. The `id` and `provenance` values in the `.golden` must also match the `.raw`.

### Regenerating

`NETLAB_GOLDEN=write cargo test -p netlab-core --test cli` is the only way to write `.golden` files. Follow this routine:

1. Edit the `.raw` file. Do not edit a `.golden` file.
2. Run the command above. It writes every `.golden` file. It also writes `web/_test/corpus-steps.json`, a copy of every case that the [JavaScript harness](#javascript-harness) replays. It skips that file if any case fails to run.
3. Read the `git diff` of the `.golden` files. Each changed line must follow from your change. If a line changes and you cannot explain it, find the cause. Do not commit it.
4. Run `cargo test -p netlab-core --test cli` without the variable. It must pass.

Regeneration records whatever the engine prints. It does not check that the output is right. A wrong output that you regenerate becomes the expected output.

## Masks

Some output changes between runs. Declare a mask in the header to replace it with a fixed token.

| Mask | Replaces | In |
|---|---|---|
| `uptime` | `H:MM:SS`, `<n>w<n>d`, `<n>d<n>h` with `<uptime>` | Engine output and `!event` lines |
| `tick` | `tick <n>`, `tick=<n>`, `t=<n>` with `tick <tick>`, `tick=<tick>`, `t=<tick>` | `!event` lines only |

Prompts, echoed input, caret lines, and counts are never masked. A mask that matches nothing fails the case. Declare only the masks the case needs.

## Gap markers

A gap marker records a command that the engine does not support yet. The case still types the command. The marker states that the engine must reject it. This keeps the list of missing commands visible and tested.

```text
## gap: <tag>
<command line>
```

`<tag>` names the deferred feature. The runner accepts only tags from its own list. `core/tests/cli/corpus/c19-gap-ledger.raw` shows the full range. To bind the marker to one word, add `tok<n>`, where `<n>` counts words from zero: `## gap: <tag> tok2`.

The marker applies to the next step. That step must be a plain line. A marker before `## script`, before another `##` directive, or at the end of the file fails the case.

### What the engine must do

The run passes only if the command does not change the device and the engine rejects it in one of three ways:

1. A caret line and one of `% Invalid input detected at '^' marker.`, `% Incomplete command.`, or `% Ambiguous command`.
2. Any other error line that starts with `%`.
3. The translate render: `Translating "..."` or `% Unknown command or computer name, or unable to find computer address`.

With `tok<n>` the check is stricter:

- `tok0` requires the translate render. The first word is unknown.
- `tok<n>` for `n` above 0 requires the caret to sit under word `n`.

The golden file shows the marker line before the typed line, so a reader sees which commands are deferred.

### When the feature ships

When the engine starts to accept the command, the line changes the device or succeeds, and the case fails with `gap closed — remove marker, write real golden`. Do these steps:

1. Delete the `## gap:` line.
2. Add the steps that show the real behavior: the command, its output, and the configuration it changes.
3. Regenerate the `.golden` files and review the diff.

A check in `properties.rs` also requires that every deferred keyword in the runner's list appears in at least one gap-marked line with the matching tag. When you add a keyword to that list, add a gap-marked line for it. When a feature ships, remove its keyword from the list in the same change.

## JavaScript harness

The cases also run in Node.js against the browser code. The harness lives in `web/_test/`. It checks that the web console (`web/cli.js`) drives the compiled WebAssembly engine in the same way as the Rust runner.

```text
node --test web/_test/   → run the harness (no install step)
```

The harness loads the prebuilt `web/pkg/` module. After you change the Rust code, run `make wasm` first. Otherwise the harness tests the old engine.

### Files

| File | Content |
|---|---|
| `corpus-steps.json` | One record per case: `id`, `src-sha`, `masks`, `steps`, and `frames`. `frames` is the expected transcript, with masks applied. |
| `rigs.mjs` | One entry per case id: `kind`, `topo`, `tick`, `ifmap`, and `startup`. It builds the fixture topology through the WebAssembly exports. |
| `frames.mjs` | Turns the engine output into the same frame lines as the Rust runner. It also applies masks and checks gap markers. |
| `spy.test.mjs` | The tests. |

The harness reads only `corpus-steps.json`. It never reads `.raw` or `.golden` files. It never writes `corpus-steps.json`. Only the Rust command in [Regenerating](#regenerating) writes it.

The record has no header fields. When you add a case, add its entry to `rigs.mjs` too. A case with no entry fails.

### What it checks

Each record runs through the real `SimShell` with a recording host. Calls to the engine go through a proxy that logs them in order. For every record, the harness checks these points:

- **Checkpoint order.** Exactly one undo checkpoint comes before each `cli_exec` or `cli_script` call. Steps that cannot change the device (`## key:`, `## tick:`, `## drain`) push no checkpoint and do not refresh or mark the lab dirty.
- **Effects.** The refresh and dirty calls match the `mutated` flag of the result. Each request for the host reaches its channel. `reload` resets the configuration and restores power. `nvram-write` and `nvram-erase` update the startup text. `rename` renames the node. `ping` and `trace` start a probe.
- **Console output.** The spans that the shell writes to the console match the engine output, in order.
- **Frame parity.** The harness renders the engine output into frames. It compares them line by line with the `frames` of the record. This is the same transcript that the `.golden` file holds.
- **Events.** No event waits undrained at the end of a case, unless the case has `## allow-events`.

Two more tests cover a deleted node, which must print `% node was deleted`, and the harness itself. The self-test breaks the ordering, the frames, and the masks on purpose and expects each break to fail.

### Relation to the Rust runner

The Rust runner is the only place that parses `.raw` files and defines the grammar. The Rust test compares the engine with the `.golden` file. The JavaScript harness compares the adapter and the compiled engine with the same frames. A case passes in both places or the two sides disagree.

### Pinned divergence

One case differs between the two sides. The Rust runner uses one session for the whole case. After a confirmed `reload`, that session stays in privileged mode. The web console replays the saved configuration after a reload. Its session returns to user mode.

In `c06-pending-dialogs`, the last `reload` line shows the prompt `R#` in the transcript and `R>` in the browser. The map `KNOWN_DIVERGENCES` in `spy.test.mjs` pins this one difference:

- The test accepts a mismatch only if it is that line, with those two prompts.
- Any other mismatch in the case fails the test.
- If the two sides agree, the test fails. Remove the pin and compare the whole record.

# IOS-style CLI engine

`netlab-core` includes an IOS-style command-line engine in `core/src/cli/`. One `Cli` session drives one device: a router, a switch, or a host.

The browser console of every simulated node runs on this engine. `web/cli.js` sends each typed line to the engine and draws the result. It does not parse commands. Every command, error, and dialog on this page applies to the browser console word for word.

This page documents the command surface, the browser console, and the honest gaps. The [README](../README.md#simulated-cli-reference) gives a short quick reference. The [WASM exports](wasm-api.md) connect the engine to JavaScript. [Corpus](corpus.md) describes the transcript tests. The [lab format](lab-format.md) describes how a lab file stores configuration and startup scripts.

## Session API

```text
Cli::new()                     → new session at user exec
cli.set_ifmap(port_names)      → register the device's interface names
cli.exec(device, line, tick)   → run one line; returns prompt, output, pending dialog
cli.help(device, text)         → `?` on the text before the cursor
cli.complete(device, text)     → Tab on the text before the cursor
cli.key(device, "ctrlz" | "ctrlc")
cli.script(device, text)       → run a pasted or startup script
```

Every call returns the new prompt. Output lines carry a class (plain, error, or OK). Some lines are requests for the host application, such as `Ping`, `NvramWrite`, `NvramErase`, `Reload`, and `Rename`. The engine does not run those requests itself.

## Mode ladder

Routers and switches use six modes. Hosts have one mode and no ladder.

| Prompt | Mode | Enter with | Leave with |
|---|---|---|---|
| `R1>` | User exec | start of session | `enable` goes up. `exit` or `logout` closes the session |
| `R1#` | Privileged exec | `enable` | `disable`, `exit`, `logout` |
| `R1(config)#` | Global config | `configure terminal` | `exit` (to `R1#`), `end` |
| `R1(config-if)#` | Interface | `interface <if>` | `exit` (to config), `end` |
| `S1(config-vlan)#` | VLAN (switch only) | `vlan <1-4094>` | `exit` (to config), `end` |
| `R1(config-router)#` | OSPF (router only) | `router ospf <1-65535>` | `exit` (to config), `end` |

- `exit` climbs one level. `end` and **Ctrl+Z** jump to privileged exec. At user or privileged exec, `exit` prints `con0 is now available` and `Press RETURN to get started.`
- **Ctrl+C** cancels an open dialog and prints `^C`.
- If a command does not exist in the current mode, the engine tries the parent modes. The mode does not change. In `(config)#`, `show running-config` works.
- `do <command>` runs a privileged exec command from any config mode.
- Interface names use the device's canonical form, such as `GigabitEthernet0/0` on a router and `FastEthernet0/1` on a switch. Short forms such as `g0/0` and `fa0/1` work.
- A router port `g0/N` is port `N` on the canvas. A switch port `fa0/N` is canvas port `e(N-1)`. For example, `fa0/2` is `e1`.
- Router interfaces start administratively down. Enter `no shutdown` on each interface you use. Switch interfaces start up.

## Abbreviation, `?`, and Tab

- Any unique prefix of a keyword works: `sh run`, `sh ip ro`, `conf t`, `en`.
- `?` lists the keywords that can come next, each with a short description. After a partial word, it lists the matching keywords.
- **Tab** completes the last word when only one keyword matches. If several match, it returns the list of candidates.
- Hidden keywords stay out of `?` output but still run.
- The session keeps the last 10 lines of interactive input. `show history` prints them. `terminal history size <0-256>` changes the limit. Script lines and dialog answers do not enter history.

## Errors

| Output | Meaning |
|---|---|
| `% Invalid input detected at '^' marker.` | A caret line marks the first word the grammar rejects. |
| `% Incomplete command.` | The line stops before a required argument. |
| `% Ambiguous command` | The prefix matches more than one keyword. |
| `Translating "foo" ...domain server (255.255.255.255)`, then `% Unknown command or computer name, or unable to find computer address` | The first word is not a command in any mode. Routers and switches only. |
| `% Unknown command or computer name, or unable to find computer address` | Same case after `no ip domain-lookup`. The `Translating` line disappears. |
| `% Feature not supported on this device` | The command parses, but this device kind rejects it. A switch gives this for `ip address`. |

## Saving, erasing, and reloading

`write memory` has no dialog. It prints `Building configuration...` and `[OK]`, then stores the running configuration as the startup configuration.

Dialogs take over the prompt until you answer. A dialog sees every line you type. Its answers do not run as commands.

| Command | Prompt | Answers |
|---|---|---|
| `copy running-config startup-config` | `Destination filename [startup-config]? ` | A blank line or `startup-config` confirms. Any other name prints `%Error opening nvram:<name> (No such file or directory)`. |
| `write erase`, `erase startup-config` | `Erasing the nvram filesystem will remove all configuration files! Continue? [confirm] ` | `y`, `yes`, or a blank line erases. `n` or `no` cancels without output. Other input asks again. |
| `reload` | `System configuration has been modified. Save? [yes/no]: ` | `y`, `yes`, or a blank line saves, then asks `Proceed with reload? [confirm] `. `n` or `no` skips the save and asks the same question. Other input asks again. |
| (reload, second step) | `Proceed with reload? [confirm] ` | `y`, `yes`, or a blank line reloads. `n` or `no` cancels. Other input asks again. |

`show startup-config` prints the saved text. With nothing saved, it prints `%% Non-volatile configuration memory is not present`.

## Scripts and paste

`cli.script` runs a block of text through the same session that a typed line uses.

- Blank lines and lines that start with `!` or `#` are skipped.
- On routers and switches, the engine adds `enable` and `configure terminal` at the top. Leading `enable` or `configure terminal` lines in your text merge into that pair.
- Each line is echoed with its prompt before its output.
- Errors collect with the line number from your text. The rest of the script still runs.
- After the last line, the session returns to the mode it had before the script. Any open dialog is cleared.

## The browser console

Double-click a simulated node, or open the **Console** tab in the bottom dock. Hosts, routers, and switches from the simulated catalog use this console. Emulated nodes keep their serial and VGA terminals.

### Keys

| Key | Action |
|---|---|
| Enter | Runs the line. An open dialog reads the line as its answer. |
| `?` | Lists the keywords that can follow the text before the cursor. The key does not insert a character. |
| Tab | Completes the last word. Several matches print as a list. |
| ↑ / ↓ | Recall earlier lines. The console keeps the last 256. The first ↑ saves the line you are typing. ↓ past the newest line restores it. |
| Ctrl+C | Cancels an open dialog. It also stops a running `ping` or `traceroute` from printing. |
| Ctrl+Z | Jumps to privileged exec, like `end`. |
| Ctrl+A / Ctrl+E | Moves the cursor to the start or the end of the line. |
| Ctrl+B / Ctrl+F | Moves the cursor back or forward one character. |
| Ctrl+U / Ctrl+W | Deletes the whole line, or the word before the cursor. |
| Ctrl+L | Clears the scrollback. The mode and the prompt stay. |
| Paste | Runs every pasted line as one script. See [Scripts and paste](#scripts-and-paste). |

- Esc then B or F, and Ctrl+R, do nothing.
- `show history` prints the engine's own history of 10 lines. The ↑ and ↓ keys use the console's longer list of 256.
- A rejected `?` or Tab draws a `^` under the first rejected word, then the standard error line.
- Each Enter and each paste adds one undo step. A command that changes configuration also refreshes the canvas and queues an autosave.

### Probes

`ping` and `traceroute` on routers and switches, and `ping` and `trace` on hosts, send real probes through the simulation. Replies print as the simulation ticks. In Simulation mode, a paused simulation prints nothing until you press play or step.

If the node is powered off or the address is invalid, the console prints `could not send probe`.

### Console states

- `connecting…` appears while the engine loads. The input stays disabled until the first prompt arrives.
- If you delete a node while its console is open, the console prints `% node was deleted` and disables the input.

### Saving, erasing, and reloading

Each simulated node has a startup script. The Inspector shows it under **Startup script**, next to the **Apply on start** box. The script is plain text in the same form as `show running-config`.

| You do | Result |
|---|---|
| `write memory` or `copy running-config startup-config` | Stores the running configuration as the startup script. Turns **Apply on start** on. |
| `write erase` or `erase startup-config` | Deletes the startup script. Turns **Apply on start** off. |
| `show startup-config` | Prints the startup script. |
| Edit the script in the Inspector | Changes the stored text at once. `show startup-config` prints the new text. |
| Click **Apply** in the Inspector | Runs the script on the running node now. This works with **Apply on start** off. The node must run. |
| Start a node | Powers it on. If **Apply on start** is on and a script exists, the script runs. |
| Open or load a lab | Builds every node from the file. Nodes with **Apply on start** on then run their script. |
| `reload` | Restores the loaded configuration, then runs the script when **Apply on start** is on. |

A reload does these things in order:

1. It restores the node to the configuration it had when the lab loaded. A node you added to the canvas returns to factory defaults.
2. It clears learned state: the ARP cache, the MAC table, NAT translations, and OSPF neighbors.
3. It keeps the link attachments and keeps the node running or stopped, as before.
4. It runs the startup script if **Apply on start** is on.

Changes you did not save with `write memory` vanish at reload. The autosave keeps the lab in the browser, but it does not change the startup script.

When a script line fails, the console prints each failure as `line N: message`. The remaining lines still run. A script that runs at node start or lab open reports its failures as a toast that names the first three.

## Command reference

`<if>` is an interface name. `<ip>` and `<mask>` are dotted-quad addresses. For ACL entries, `<addr>` is `any`, `host <ip>`, or `<net> <wildcard>`.

### Routers and switches: exec and privileged exec

User exec accepts the first group. Privileged exec accepts both groups.

```text
enable | exit | logout
ping <ip>                      traceroute <ip>
terminal history size <0-256>
show arp | history | interfaces | running-config | startup-config | status | version
show ip access-lists | dhcp binding | interface brief | nat translations
show ip ospf [neighbor | interface] | protocols | route
show logging buffered
do <exec command>
```

Switches add `show mac address-table`, `show switchport`, and `show vlan [brief]`.

```text
disable
configure terminal
copy running-config startup-config
write memory | write erase
erase startup-config
reload
clear arp-cache | counters [<if>]
clear ip nat translation | ip ospf process | ip route *
clear mac address-table dynamic
```

### Global config

```text
hostname <name>
interface <if>
access-list <1-2699> permit|deny ip|icmp|tcp|udp <addr> <addr>
ip route <net> <mask> <next-hop> [<1-255>]
ip routing | ip domain-lookup | ip proxy-arp | ip directed-broadcast
ip dhcp excluded-address <low> [<high>]
ip nat inside source static <inside-local> <inside-global>
service dhcp
end | exit | do <exec command>
```

- Routers add `router ospf <1-65535>`.
- Switches add `vlan <1-4094>`.
- `no <command>` and `default <command>` negate the forms above: `hostname`, `access-list <n> [entry]`, `ip route`, the `ip` toggles, `ip dhcp excluded-address`, `ip nat inside source static`, `service dhcp`, and `router ospf <n>` (router) or `vlan <n>` (switch).
- `no hostname` and `default hostname` restore the default device name.
- `no access-list <n>` deletes the whole list. Add an entry to delete one line.

### Interface config

```text
description <text>
ip address <ip> <mask>                   (router)
ip access-group <1-2699> in|out          (router)
ip nat inside | outside                  (router)
shutdown
switchport mode access | trunk           (switch)
switchport access vlan <1-4094>          (switch)
switchport trunk allowed vlan <list>     (switch)
no <any of the above> | default <any of the above>
end | exit | do <exec command>
```

### VLAN config (switch)

```text
name <word>
state active | suspend
no name | no state
end | exit | do <exec command>
```

### OSPF config (router)

```text
network <net> <wildcard> area <n>
router-id <ip>
no network <net> <wildcard> area <n> | no router-id
end | exit | do <exec command>
```

### Hosts

Hosts have one flat command set and the prompt `PC1>`. There is no `enable` or `configure terminal`.

```text
ip <addr/len> [<gateway>]
ip <addr> <mask> [<gateway>]
ip dhcp
ping <ip> | trace <ip>
arp | show arp | show ip
set pcname <name>
```

The words `traffic`, `nc`, `curl`, `server`, and `nslookup` parse. Typed alone, each prints `% <word>: not supported in this lab`. With arguments, they print `% Invalid input detected at '^' marker.`

## Unsupported commands

These commands do not exist in the grammar. The engine rejects them with the error shown. This list is not complete. Anything absent from the command reference is unsupported.

| You type | Result |
|---|---|
| `ip access-list extended <name>` or `ip access-list standard <name>` (named ACLs) | `% Invalid input detected at '^' marker.` Use numbered `access-list <1-2699> ...`. |
| `access-list 101 permit tcp any any eq 80` (port match) | `% Invalid input detected at '^' marker.` ACL entries match protocol, source, and destination only. |
| `ip dhcp pool <name>` | `% Invalid input detected at '^' marker.` Only `ip dhcp excluded-address` and `service dhcp` exist. |
| `interface vlan 10` (SVI) | `% Invalid input detected at '^' marker.` Switch interfaces take no VLAN interface. |
| `interface Loopback0`, `interface range ...` | `% Invalid input detected at '^' marker.` |
| `terminal length 0` | `% Invalid input detected at '^' marker.` Only `terminal history size` exists. |
| `ping <ip> repeat 5`, `ping <ip> size 100`, and other extended ping options | `% Invalid input detected at '^' marker.` `ping` takes one address. A hidden `ping -c <1-100> <ip>` sets a probe count. |
| `ip helper-address <ip>` | `% Invalid input detected at '^' marker.` |
| `ip ospf <pid> area <n>`, `ip address dhcp` | `% Invalid input detected at '^' marker.` Enable OSPF with `network ... area ...` under `router ospf`. |
| `switchport trunk native vlan <n>` | `% Invalid input detected at '^' marker.` |
| `enable secret <text>` | `% Invalid input detected at '^' marker.` |
| `username ...`, `line ...`, `banner ...` | The `Translating ...` line and `% Unknown command or computer name, or unable to find computer address`. |
| `router ospf <n>` on a switch, `vlan <n>` on a router | Same unknown-command output. |
| `show access-lists` | `% Invalid input detected at '^' marker.` Use `show ip access-lists`. |

## Forms from the earlier browser shell

Older lab notes and startup scripts can use forms from the browser shell that the console used before. The engine rejects them. This table shows the engine form to use.

| Earlier form | In the engine |
|---|---|
| Host: `ip eth1 <addr/len>`, `ip dns <ip>`, `clear ip`, `show route`, `hostname <name>`, `reload` | `% Invalid input detected at '^' marker.` A host takes `ip <addr/len> [<gateway>]`. Rename with `set pcname <name>`. |
| Host: `traceroute <ip>`, `ping <ip> -c <n>` | `% Invalid input detected at '^' marker.` Use `trace <ip>` and `ping <ip>`. |
| Host: `nslookup`, `curl`, `nc`, `server`, `traffic` | Typed alone, `% <word>: not supported in this lab`. With arguments, the invalid-input error. |
| Router or switch: `ip address <if> <addr/len>`, `switchport mode <if> access` on one line | Open the interface first with `interface <if>`. Then use `ip address <ip> <mask>` or `switchport mode access`. Masks use dotted-quad form. |
| Router: `ip route <prefix/len> <next-hop> [<if>]`, `no ip route <index>` | `ip route <net> <mask> <next-hop> [<1-255>]` in global config. Remove a route with `no ip route <net> <mask> <next-hop>`. |
| Router: `router ospf network <prefix> <wild> area <id>`, `ip ospf <if> area <n>` | `router ospf <pid>`, then `network <net> <wildcard> area <n>`. |
| Router: `ip nat inside source static tcp\|udp <ip> <port> <port>` (port forward) | `% Invalid input detected at '^' marker.` Only the 1:1 form `ip nat inside source static <inside-local> <inside-global>` exists. |
| Router: `show ip nat` | `% Incomplete command.` Use `show ip nat translations`. |
| Router or switch: `traffic <ip> tcp\|udp\|icmp <port> -c <n>`, `nslookup` | The `Translating ...` line, then the unknown-command error. |

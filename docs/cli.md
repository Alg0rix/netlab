# IOS-style CLI engine

`netlab-core` includes an IOS-style command-line engine in `core/src/cli/`. One `Cli` session drives one device: a router, a switch, or a host.

This page documents the engine's command surface. The browser console ships its own shell in `web/cli.js`. The [README](../README.md#simulated-cli-reference) lists that shell's commands. The two grammars differ. For example, the engine takes `ip route <net> <mask> <next-hop>`, and the browser shell takes `ip route <prefix/len> <next-hop>`.

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
| `copy running-config startup-config` | `Destination filename [startup-config]? ` | Any answer, or a blank line, confirms. The engine ignores the typed text. It does not accept another file name. |
| `write erase`, `erase startup-config` | `Erasing the nvram filesystem will remove all configuration files! Continue? [confirm] ` | `y`, `yes`, or a blank line erases. `n` or `no` cancels without output. Other input asks again. |
| `reload` | `System configuration has been modified. Save? [yes/no]: ` | `y` or blank saves, then asks `Proceed with reload? [confirm] `. `n` skips the save and asks the same question. |
| (reload, second step) | `Proceed with reload? [confirm] ` | `y`, `yes`, or blank reloads. `n` or `no` cancels. |

`show startup-config` prints the saved text. With nothing saved, it prints `%% Non-volatile configuration memory is not present`.

## Scripts and paste

`cli.script` runs a block of text through the same session that a typed line uses.

- Blank lines and lines that start with `!` or `#` are skipped.
- On routers and switches, the engine adds `enable` and `configure terminal` at the top. Leading `enable` or `configure terminal` lines in your text merge into that pair.
- Each line is echoed with its prompt before its output.
- Errors collect with the line number from your text. The rest of the script still runs.
- After the last line, the session returns to the mode it had before the script. Any open dialog is cleared.

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

# Lab file format

netlab saves a lab as `.netlab.json` or as `.netlab.yaml`. Both files hold the same topology. **File → Save YAML lab…** writes the YAML form. **Open** reads either form. This page describes the YAML form for people who write or edit it by hand. It also states what the app does with each field when it opens a lab.

The [CLI page](cli.md#saving-erasing-and-reloading) explains how `write memory`, `reload`, and the startup script use these fields.

## Document layout

```yaml
name: my lab
nodes:
  - id: 1
    template: router
    name: R1
    x: 120
    y: 80
    ports:
      - { ip: 10.0.0.1, mask: 255.255.255.0 }
links:
  - { a: 1, ap: 0, b: 2, bp: 0, delay: 4, jitter: 0, loss: 0, mtu: 0, speed: 1G }
view: { x: 0, y: 0, z: 1 }
notes: []
shapes: []
```

- The loader ignores unknown keys.
- A dotted key such as `meta.startup:` and a nested block such as `meta:` with `startup:` under it mean the same thing.
- A JSON file loads as written. Only the YAML loader fills in the defaults that this page lists.
- Save leaves out most values that equal their default.

## Node keys

| Key | Meaning |
|---|---|
| `id` | Node number. Links refer to it. |
| `template` | Node type, such as `router`, `switch`, or `pc`. JSON files call this key `tpl`. |
| `name`, `x`, `y` | Display name and canvas position. |
| `powered` | `false` starts the node powered off. Absent means powered on. |
| `portCount` | Number of ports. |
| `services` | A list such as `[dns, http]`. A server node starts the listed services. |
| `resolver` | DNS server address for the node. |
| `dns` | Name records. Each row is `{ name: <name>, ip: <address> }`. |
| `ports` | One row per port, in port order. See [Ports](#ports). |
| `routes` | Static routes. See [Routes](#routes). |
| `mac`, `arp` | Table rows. See [MAC and ARP rows](#mac-and-arp-rows). |
| `vlans` | VLAN names and states. Switches only. |
| `toggles` | Device switches. See [Toggles](#toggles). |
| `ospf` | OSPF settings. See [OSPF](#ospf). |
| `meta.startup`, `meta.startupBoot` | The startup script. See [Startup script](#startup-script). |

Leave `uid` as it is. It is an internal storage key.

## Startup script

`meta.startup` holds the text that `write memory` saves and that **Startup script** in the Inspector shows. Write it as a YAML block scalar:

```yaml
    meta.startup: |-
      hostname R1
      interface g0/0
       ip address 10.0.0.1 255.255.255.0
       no shutdown
    meta.startupBoot: true
```

- `|-` keeps the text without a final newline. Save writes this form.
- `|` keeps one final newline. `|+` keeps every trailing blank line. Save writes `|+` when the script ends in a newline.
- `>` folds each line break into a space. Use `|` for a script.
- A one-line quoted string also works: `meta.startup: "hostname R1"`.
- `meta.startup` must be text. A mapping or a list gives no script.
- An absent key and an empty script both mean: no script.

`meta.startupBoot: true` is the **Apply on start** box. Absent means `false`. The script runs when the node starts, when the lab opens, and after a reload, but only when both of these are true:

1. `meta.startupBoot` is `true`.
2. `meta.startup` is text.

The script runs through the same path as a paste, so the rules in [Scripts and paste](cli.md#scripts-and-paste) apply. Router interfaces start administratively down. Add `no shutdown` for each interface the script uses.

## Ports

Each row in `ports` describes one port, in port order. `- ~` and `- {}` both leave the port at its defaults.

| Key | Absent means | Notes |
|---|---|---|
| `ip`, `mask` | No address | Give both. |
| `nat` | No NAT role | `inside` or `outside`. |
| `mode` | Access | `trunk` makes the port a trunk. `trunk: true` works too. |
| `access_vlan` | 1 | |
| `native_vlan` | 1 | |
| `allowed` | No list | A VLAN list as text, such as `10,20`. |
| `helper` | No relay | DHCP relay server address. |
| `in_acl`, `out_acl` | No access list | Access-list number. |
| `ospf_area` | OSPF off on this port | |
| `admin_up` | Router: `false`. Switch and host: `true` | `false` shuts the port down. |
| `method` | `none` | `none`, `manual`, or `dhcp`. Any other word becomes `none`. |
| `stats.rx`, `stats.tx`, `stats.rx_err`, `stats.tx_err`, `stats.drops`, `stats.cleared_tick` | 0 | Counters that `show interfaces` prints. A bad number becomes 0. |

## Routes

```yaml
    routes:
      - { prefix: 10.9.0.0, mask: 255.255.255.0, next_hop: 10.0.0.2, out_port: 0, ad: 1 }
```

| Key | Absent means |
|---|---|
| `prefix`, `next_hop` | Required. The loader drops a row without them. |
| `mask` | No mask. |
| `out_port` | 0 |
| `proto` | `static`. Also `connected` or `ospf`. The letters `c`, `s`, and `o` work. |
| `ad` | The default for `proto`: 0 connected, 1 static, 110 OSPF. |
| `metric`, `learned_tick` | 0 |

Save writes static routes only. Connected routes come from the interface addresses. OSPF routes come from the protocol.

## MAC and ARP rows

| Table | Keys | Absent means |
|---|---|---|
| `mac` | `mac`, `port`, `vlan`, `created_tick`, `etype` | `vlan` 1. `created_tick` 0. `etype` `dynamic`. |
| `arp` | `ip`, `mac`, `port`, `created_tick`, `etype` | `created_tick` 0. `etype` `dynamic`. |

`etype` is `dynamic` or `static`. Any other word becomes `dynamic`. A row without `mac`, or without `ip` in `arp`, is dropped.

## VLANs

Write one key per VLAN, or use a nested block:

```yaml
    vlans.10.name: SALES
    vlans.20.state: suspend
    vlans.30: {}
```

- `state` is `active` or `suspend`. Absent means `active`.
- Absent `name` means `VLAN` plus the number padded to four digits, such as `VLAN0030`.
- `vlans.30: {}` creates VLAN 30 with the defaults.
- VLANs 1, 1002, 1003, 1004, and 1005 always exist. Their names are `default`, `fddi-default`, `token-ring`, `fddinet`, and `trnet`.
- A VLAN number outside 1 to 4094 is ignored.
- Routers and hosts ignore `vlans`.

## Toggles

```yaml
    toggles.proxy_arp: false
```

An absent toggle takes the default for the node type. A value that is not `true` or `false` also takes the default.

| Toggle | Router | Switch | Host |
|---|---|---|---|
| `domain_lookup` | true | true | true |
| `proxy_arp` | true | true | false |
| `directed_broadcast` | false | false | false |
| `ip_routing` | true | false | false |
| `routing_enabled` | true | true | false |

## OSPF

```yaml
    ospf.pid: 1
    ospf.router_id: 1.1.1.1
    ospf.networks:
      - { net: 10.0.0.0, wild: 0.0.0.255, area: 0 }
```

- `ospf.pid` is required. Without it, the loader drops the whole block.
- Without `ospf.router_id`, the router derives its ID from its highest interface address. Save writes the key only when you set it.
- A network row needs `net` and `wild`. Absent `area` means 0.

## What the app applies when it opens a lab

The loader reads every field above and applies all of them to the node before it records the node baseline:

- `ip`, `mask`, `nat`, `mode`, `access_vlan`, `native_vlan`, `allowed`, `helper`, `in_acl`, `out_acl`, `ospf_area`
- `admin_up`, `method`, and `stats.*` on each port
- Static routes, including `ad`, `metric`, and `learned_tick`
- `mac`, `arp`, `vlans`, and `toggles`
- `ospf.pid`, `ospf.router_id`, and `ospf.networks`
- `acls`, `static_nat`, `services`, `resolver`, `dns`
- `powered`, `meta.startup`, and `meta.startupBoot`

A reload returns the node to that authored state, but clears learned state the way a real reload does: `mac` and `arp` rows do not survive it. `toggles.routing_enabled` never changes after the node is created.

## What YAML save does not write

YAML save leaves these out:

- Access lists (`acls`), `in_acl`, `out_acl`, and `static_nat`.
- `service dhcp` and `ip dhcp excluded-address` settings.
- Console sessions, command history, scrollback, and OSPF neighbor state.

The YAML loader still reads `acls`, `in_acl`, `out_acl`, and `static_nat` from a file you write by hand. To keep this configuration in a saved YAML lab, write the commands in `meta.startup` and set `meta.startupBoot: true`. A `.netlab.json` save keeps access lists and static NAT.

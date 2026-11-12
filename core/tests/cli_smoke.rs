use netlab_core::cli::types::CompleteResult;
use netlab_core::cli::{Cli, HelpResult, Intent, Op};
use netlab_core::{Device, DeviceKind, SimEvent};

fn router() -> Device {
    Device::new(1, "R1".into(), DeviceKind::Router, 0.0, 0.0, vec![1, 2])
}
fn switch() -> Device {
    Device::new(2, "S1".into(), DeviceKind::Switch, 0.0, 0.0, vec![10, 11])
}
fn host() -> Device {
    Device::new(3, "PC1".into(), DeviceKind::Host, 0.0, 0.0, vec![20])
}

fn txt(ops: &[Op]) -> String {
    ops.iter()
        .filter_map(|o| match o {
            Op::Text { text, .. } => Some(text.clone()),
            Op::Echo { text } => Some(text.clone()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn exec(c: &mut Cli, d: &mut Device, line: &str) -> netlab_core::cli::CliResult {
    let mut ev: Vec<SimEvent> = Vec::new();
    c.exec(d, line, 0, &mut ev)
}

#[test]
fn router_mode_lattice_and_config() {
    let mut d = router();
    let mut c = Cli::new();
    c.set_ifmap(&d.port_names()).unwrap();

    let r = exec(&mut c, &mut d, "enable");
    assert_eq!(r.prompt, "R1#");
    let r = exec(&mut c, &mut d, "configure terminal");
    assert_eq!(r.prompt, "R1(config)#");
    let r = exec(&mut c, &mut d, "hostname Core");
    assert!(r.mutated);
    assert_eq!(d.name, "Core");
    let r = exec(&mut c, &mut d, "interface gigabitethernet0/0");
    assert_eq!(r.prompt, "Core(config-if)#");
    let r = exec(&mut c, &mut d, "ip address 10.0.0.1 255.255.255.0");
    assert!(r.mutated, "{}", txt(&r.out));
    assert_eq!(d.ports[0].ip, Some(([10, 0, 0, 1], [255, 255, 255, 0])));
    let r = exec(&mut c, &mut d, "no shutdown");
    assert!(r.mutated);
    assert!(d.ports[0].admin_up);
    // exit climbs one parent
    let r = exec(&mut c, &mut d, "exit");
    assert_eq!(r.prompt, "Core(config)#");
    // end jumps to priv exec
    let r = exec(&mut c, &mut d, "end");
    assert_eq!(r.prompt, "Core#");
    let r = exec(&mut c, &mut d, "disable");
    assert_eq!(r.prompt, "Core>");
    let r = exec(&mut c, &mut d, "exit");
    assert_eq!(r.prompt, "Core>");
    assert!(txt(&r.out).contains("con0 is now available"));
}

#[test]
fn router_show_and_nvgen() {
    let mut d = router();
    let mut c = Cli::new();
    exec(&mut c, &mut d, "enable");
    exec(&mut c, &mut d, "conf t");
    exec(&mut c, &mut d, "int g0/0");
    exec(&mut c, &mut d, "ip addr 10.0.0.1 255.255.255.0");
    exec(&mut c, &mut d, "no shut");
    exec(&mut c, &mut d, "desc uplink");
    exec(&mut c, &mut d, "end");
    let r = exec(&mut c, &mut d, "show running-config");
    let t = txt(&r.out);
    assert!(t.contains("interface GigabitEthernet0/0"), "{t}");
    assert!(t.contains("ip address 10.0.0.1 255.255.255.0"), "{t}");
    assert!(t.contains("description uplink"), "{t}");
    assert!(t.contains("no shutdown"), "{t}");
    let r = exec(&mut c, &mut d, "show ip interface brief");
    let t = txt(&r.out);
    assert!(t.contains("GigabitEthernet0/0"), "{t}");
    assert!(t.contains("10.0.0.1"), "{t}");
    // a shut port renders "administratively down" + one pad space + "down"
    exec(&mut c, &mut d, "conf t");
    exec(&mut c, &mut d, "int g0/1");
    exec(&mut c, &mut d, "shutdown");
    exec(&mut c, &mut d, "end");
    let r = exec(&mut c, &mut d, "show ip interface brief");
    let t = txt(&r.out);
    let line = t.lines().find(|l| l.contains("GigabitEthernet0/1")).expect(&t);
    assert!(line.contains("administratively down down"), "{line}");
    let r = exec(&mut c, &mut d, "write memory");
    let t = txt(&r.out);
    assert!(t.contains("Building configuration"), "{t}");
    assert!(r.out.iter().any(|o| matches!(
        o,
        Op::Intent(Intent::NvramWrite { .. })
    )));
}

#[test]
fn errors_ambiguous_incomplete_invalid() {
    let mut d = router();
    let mut c = Cli::new();
    exec(&mut c, &mut d, "enable");
    // incomplete
    let r = exec(&mut c, &mut d, "show");
    assert!(txt(&r.out).contains("% Incomplete command."), "{}", txt(&r.out));
    // ambiguous at priv exec: "e" prefixes enable + exit + erase
    let r = exec(&mut c, &mut d, "e");
    let t = txt(&r.out);
    assert!(t.contains("Ambiguous command"), "{t}");
    // invalid word mid-command: show zz — NOT translated (first token ok)
    let r = exec(&mut c, &mut d, "show zz");
    let t = txt(&r.out);
    assert!(t.contains("Invalid input detected"), "{t}");
    assert!(t.contains("^"), "{t}");
    assert!(!t.contains("Translating"), "{t}");
    // wholly unknown first token → translate (domain_lookup default on)
    let r = exec(&mut c, &mut d, "bogusbox");
    let t = txt(&r.out);
    assert!(t.contains("Translating \"bogusbox\""), "{t}");
    assert!(t.contains("Unknown command or computer name"), "{t}");
    // no foo also refuses translation
    exec(&mut c, &mut d, "conf t");
    let r = exec(&mut c, &mut d, "no frobnicate");
    let t = txt(&r.out);
    assert!(t.contains("Invalid input detected"), "{t}");
    assert!(!t.contains("Translating"), "{t}");
}

#[test]
fn pending_dialogs_and_keys() {
    let mut d = router();
    let mut c = Cli::new();
    exec(&mut c, &mut d, "enable");
    // reload arms a pending dialog: save? → proceed?
    let r = exec(&mut c, &mut d, "reload");
    assert!(r.pending.is_some(), "reload should arm a dialog");
    let p = r.pending.unwrap().prompt;
    assert!(p.ends_with(' '), "pending prompt has one trailing space");
    let r = exec(&mut c, &mut d, "no");
    // after declining the save leg, the proceed question must arm
    assert!(r.pending.is_some(), "save leg should chain to proceed");
    assert!(r.pending.unwrap().prompt.contains("Proceed"), "proceed leg");
    let r = exec(&mut c, &mut d, "no");
    assert!(r.pending.is_none());
    assert!(!r.out.iter().any(|o| matches!(o, Op::Intent(Intent::Reload))));
    // consumed lines must not enter history
    assert_eq!(c.history.back().map(|s| s.as_str()), Some("reload"));
    // arm again, abort with ctrl-z (silent)
    let r = exec(&mut c, &mut d, "reload");
    assert!(r.pending.is_some());
    let mut ev = Vec::new();
    let r = c.key(&mut d, "ctrlz", 0, &mut ev);
    assert!(r.out.is_empty(), "ctrl-z is silent");
    assert!(c.pending.is_none());
    assert_eq!(r.prompt, "R1#", "ctrl-z in priv exec does not change mode");
    // ctrl-c emits ^C
    let r = c.key(&mut d, "ctrlc", 0, &mut ev);
    assert!(txt(&r.out).contains("^C"));
    // ctrl-z in config mode returns to priv exec
    exec(&mut c, &mut d, "conf t");
    let r = c.key(&mut d, "ctrlz", 0, &mut ev);
    assert_eq!(r.prompt, "R1#");
    // write erase arms erase dialog
    let r = exec(&mut c, &mut d, "write erase");
    assert!(r.pending.is_some());
}

#[test]
fn history_and_terminal_size() {
    let mut d = router();
    let mut c = Cli::new();
    exec(&mut c, &mut d, "enable");
    exec(&mut c, &mut d, "bogus-cmd");
    exec(&mut c, &mut d, "");
    assert_eq!(c.history.len(), 2, "empty lines never land in history");
    exec(&mut c, &mut d, "terminal history size 1");
    assert_eq!(c.history.len(), 1);
    exec(&mut c, &mut d, "terminal history size 0");
    exec(&mut c, &mut d, "enable");
    assert_eq!(c.history.len(), 0, "size 0 disables history");
}

#[test]
fn host_flat_root() {
    let mut d = host();
    let mut c = Cli::new();
    let r = exec(&mut c, &mut d, "ip 10.0.0.2/24 10.0.0.1");
    assert!(r.mutated, "{}", txt(&r.out));
    assert_eq!(d.ports[0].ip, Some(([10, 0, 0, 2], [255, 255, 255, 0])));
    let r = exec(&mut c, &mut d, "ping 10.0.0.1");
    assert!(r.out.iter().any(|o| matches!(o, Op::Intent(Intent::Ping { .. }))));
    // wired host tools want their args
    for tool in ["nc", "curl", "nslookup", "server"] {
        let r = exec(&mut c, &mut d, tool);
        assert!(
            txt(&r.out).contains("% Incomplete command"),
            "{tool}: {}",
            txt(&r.out)
        );
    }
    // traffic stays a stub for now
    let r = exec(&mut c, &mut d, "traffic");
    assert!(
        txt(&r.out).contains("% traffic: not supported in this lab"),
        "{}",
        txt(&r.out)
    );
    // resolver set + readback
    exec(&mut c, &mut d, "ip dns 8.8.8.8");
    assert_eq!(d.resolver, Some([8, 8, 8, 8]));
    let r = exec(&mut c, &mut d, "ip dns");
    assert!(txt(&r.out).contains("8.8.8.8"), "{}", txt(&r.out));
    // service flags + A record
    let r = exec(&mut c, &mut d, "server dns on");
    assert!(r.mutated);
    assert!(d.svc_dns);
    let r = exec(&mut c, &mut d, "server dns a web.local 10.0.0.1");
    assert!(r.mutated);
    assert!(d.dns_records.iter().any(|(n, ip)| n == "web.local" && *ip == [10, 0, 0, 1]));
    // nc/curl parse and report, even with nowhere to send
    let r = exec(&mut c, &mut d, "nc -u 10.0.0.1 53 hello");
    assert!(txt(&r.out).contains("UDP 10.0.0.1:53"), "{}", txt(&r.out));
    let r = exec(&mut c, &mut d, "curl 10.0.0.1:8080");
    assert!(txt(&r.out).contains("10.0.0.1:8080"), "{}", txt(&r.out));
    let r = exec(&mut c, &mut d, "nslookup web.local");
    assert!(
        txt(&r.out).contains("no address or DNS server")
            || txt(&r.out).contains("web.local"),
        "{}",
        txt(&r.out)
    );
    // no router commands on a host
    let r = exec(&mut c, &mut d, "enable");
    let t = txt(&r.out);
    assert!(t.contains("Invalid input"), "{t}");
    let r = exec(&mut c, &mut d, "show ip");
    assert!(txt(&r.out).contains("10.0.0.2"), "{}", txt(&r.out));
    // prompt stays flat
    assert_eq!(r.prompt, "PC1>");
}

#[test]
fn switch_vlan_and_ports() {
    let mut d = switch();
    let mut c = Cli::new();
    exec(&mut c, &mut d, "enable");
    exec(&mut c, &mut d, "conf t");
    let r = exec(&mut c, &mut d, "vlan 10");
    assert_eq!(r.prompt, "S1(config-vlan)#");
    let r = exec(&mut c, &mut d, "name USERS");
    assert!(r.mutated);
    let r = exec(&mut c, &mut d, "exit");
    assert_eq!(r.prompt, "S1(config)#");
    let r = exec(&mut c, &mut d, "interface fa0/1");
    assert_eq!(r.prompt, "S1(config-if)#");
    let r = exec(&mut c, &mut d, "switchport access vlan 10");
    assert!(r.mutated, "{}", txt(&r.out));
    assert_eq!(d.ports[0].access_vlan, 10);
    exec(&mut c, &mut d, "end");
    let r = exec(&mut c, &mut d, "show vlan");
    let t = txt(&r.out);
    assert!(t.contains("USERS"), "{t}");
    // `router` is absent from every switch root — the word is wholly
    // unknown, so the honest result is the translate path, not a gate msg
    exec(&mut c, &mut d, "conf t");
    let r = exec(&mut c, &mut d, "router ospf 1");
    let t = txt(&r.out);
    assert!(t.contains("Translating \"router\""), "{t}");
    assert!(t.contains("Unknown command or computer name"), "{t}");
    // a verb that exists but fails mid-command does NOT translate
    let r = exec(&mut c, &mut d, "ip routing frob");
    assert!(txt(&r.out).contains("Invalid input"), "{}", txt(&r.out));
}

#[test]
fn help_and_complete() {
    let mut d = router();
    let mut c = Cli::new();
    exec(&mut c, &mut d, "enable");
    // ? at empty → rows listing heads incl <cr>-less commands
    let h = c.help(&d, "");
    match h {
        HelpResult::Rows { rows, .. } => {
            let words: Vec<&str> = rows.iter().map(|r| r.word.as_str()).collect();
            assert!(words.contains(&"enable"), "{words:?}");
            assert!(words.contains(&"show"), "{words:?}");
            assert!(words.contains(&"configure"), "{words:?}");
            // hidden ping -c subtree must not leak
            assert!(!words.iter().any(|w| w.contains("-c")), "{words:?}");
        }
        other => panic!("expected rows, got {other:?}"),
    }
    // trailing space after leaf → <cr>
    let h = c.help(&d, "enable ");
    match h {
        HelpResult::Inline { inline, .. } => assert!(inline.iter().any(|s| s == "<cr>")),
        other => panic!("expected inline <cr>, got {other:?}"),
    }
    // tab complete: sh → show
    match c.complete(&d, "sh") {
        CompleteResult::Line { line } => assert_eq!(line, "show"),
        other => panic!("expected line, got {other:?}"),
    }
    // ambiguous prefix → list
    match c.complete(&d, "e") {
        CompleteResult::List { list } => assert!(list.len() > 1),
        CompleteResult::Line { line } => assert!(line.starts_with('e')),
        other => panic!("expected list/line, got {other:?}"),
    }
    // help is current-mode only — config-if rows come from config-if root
    exec(&mut c, &mut d, "conf t");
    exec(&mut c, &mut d, "int g0/0");
    let h = c.help(&d, "");
    match h {
        HelpResult::Rows { rows, .. } => {
            let words: Vec<&str> = rows.iter().map(|r| r.word.as_str()).collect();
            assert!(words.contains(&"ip"), "{words:?}");
            assert!(words.contains(&"shutdown"), "{words:?}");
        }
        _ => panic!("rows expected"),
    }
}

#[test]
fn script_prelude_and_restore() {
    let mut d = router();
    let mut c = Cli::new();
    let mut ev = Vec::new();
    let text = "!\nenable\nconfigure terminal\nhostname Scripted\ninterface g0/0\nip address 10.9.0.1 255.255.255.0\nno shutdown\nend\n";
    let r = c.script(&mut d, text, 0, &mut ev);
    assert!(r.errors.is_empty(), "{:?}", r.errors);
    assert!(r.mutated);
    assert_eq!(d.name, "Scripted");
    // mode restored after script
    assert_eq!(c.node, netlab_core::cli::types::ModeId::Exec);
    // script lines never enter history
    assert!(c.history.is_empty());
    // mid-script error collects lineno
    let r = c.script(&mut d, "enable\nconf t\ninterface g9/9\n", 0, &mut ev);
    assert!(!r.errors.is_empty());
    assert!(r.errors.iter().any(|e| e.lineno == 3), "{:?}", r.errors);
}

#[test]
fn translate_gating_domain_lookup() {
    let mut d = router();
    let mut c = Cli::new();
    exec(&mut c, &mut d, "enable");
    exec(&mut c, &mut d, "conf t");
    exec(&mut c, &mut d, "no ip domain-lookup");
    exec(&mut c, &mut d, "end");
    let r = exec(&mut c, &mut d, "bogusbox");
    let t = txt(&r.out);
    assert!(!t.contains("Translating"), "{t}");
    assert!(t.contains("Unknown command or computer name"), "{t}");
}

#[test]
fn do_wrapper_and_walkup() {
    let mut d = router();
    let mut c = Cli::new();
    exec(&mut c, &mut d, "enable");
    exec(&mut c, &mut d, "conf t");
    // `do show` inside config runs the priv-exec command
    let r = exec(&mut c, &mut d, "do show version");
    let t = txt(&r.out);
    assert!(!t.contains("Invalid input"), "{t}");
    // mode-changing heads are NOT under `do` — each stalls at token 1
    for bad in ["do enable", "do configure terminal", "do exit", "do end", "do do show version"] {
        let r = exec(&mut c, &mut d, bad);
        let t = txt(&r.out);
        assert!(t.contains("Invalid input"), "{bad}: {t}");
    }
    // walk-up for execution: `show` resolves from config without `do`
    let r = exec(&mut c, &mut d, "show version");
    let t = txt(&r.out);
    assert!(!t.contains("Invalid input"), "{t}");
    // `end` is not installed at priv exec → error, not a mode change
    let r = exec(&mut c, &mut d, "end");
    assert_eq!(r.prompt, "R1#");
    exec(&mut c, &mut d, "exit");
    let r = exec(&mut c, &mut d, "end");
    let t = txt(&r.out);
    assert!(
        t.contains("Unknown command or computer name"),
        "end at exec is a wholly unknown word → translate: {t}"
    );
}

#[test]
fn acl_nat_dhcp_ospf_inventory() {
    let mut d = router();
    let mut c = Cli::new();
    exec(&mut c, &mut d, "enable");
    exec(&mut c, &mut d, "conf t");
    // ACL
    let r = exec(&mut c, &mut d, "access-list 10 permit ip any any");
    assert!(r.mutated, "{}", txt(&r.out));
    let r = exec(&mut c, &mut d, "access-list 10 deny icmp host 10.0.0.5 any");
    assert!(r.mutated, "{}", txt(&r.out));
    // static NAT + interface roles
    let r = exec(&mut c, &mut d, "ip nat inside source static 192.168.1.10 203.0.113.10");
    assert!(r.mutated, "{}", txt(&r.out));
    exec(&mut c, &mut d, "interface g0/0");
    let r = exec(&mut c, &mut d, "ip nat inside");
    assert!(r.mutated, "{}", txt(&r.out));
    // access-group with {in|out} argv
    let r = exec(&mut c, &mut d, "ip access-group 10 in");
    assert!(r.mutated, "{}", txt(&r.out));
    exec(&mut c, &mut d, "exit");
    // DHCP
    let r = exec(&mut c, &mut d, "ip dhcp excluded-address 192.168.1.1 192.168.1.20");
    assert!(r.mutated, "{}", txt(&r.out));
    let r = exec(&mut c, &mut d, "service dhcp");
    assert!(r.mutated, "{}", txt(&r.out));
    // OSPF
    let r = exec(&mut c, &mut d, "router ospf 1");
    assert_eq!(r.prompt, "R1(config-router)#");
    let r = exec(&mut c, &mut d, "network 10.0.0.0 0.0.0.255 area 0");
    assert!(r.mutated, "{}", txt(&r.out));
    let r = exec(&mut c, &mut d, "router-id 1.1.1.1");
    assert!(r.mutated, "{}", txt(&r.out));
    exec(&mut c, &mut d, "end");
    // nvgen reflects the families in K14 order
    let r = exec(&mut c, &mut d, "show running-config");
    let t = txt(&r.out);
    let acl_i = t.find("access-list 10").expect(&t);
    let ospf_i = t.find("router ospf 1").expect(&t);
    let nat_i = t.find("ip nat inside source static").expect(&t);
    let dhcp_i = t.find("ip dhcp excluded-address").expect(&t);
    assert!(ospf_i < acl_i, "ospf before acl in K14: {t}");
    assert!(acl_i < nat_i, "acl before nat: {t}");
    assert!(nat_i < dhcp_i, "nat before dhcp: {t}");
    // clears exist and do not mark mutated
    for cl in ["clear arp-cache", "clear ip nat translation", "clear ip ospf process", "clear ip route *", "clear counters"] {
        let r = exec(&mut c, &mut d, cl);
        let t = txt(&r.out);
        assert!(!r.mutated, "{cl} must not mutate");
        assert!(!t.contains("Invalid input"), "{cl}: {t}");
    }
    // negations
    exec(&mut c, &mut d, "conf t");
    let r = exec(&mut c, &mut d, "no access-list 10");
    assert!(r.mutated, "removing a present acl mutates: {}", txt(&r.out));
    assert!(!txt(&r.out).contains("Invalid"), "{}", txt(&r.out));
    let r = exec(&mut c, &mut d, "no router ospf 1");
    assert!(r.mutated, "removing a running ospf process mutates");
    assert_eq!(r.prompt, "R1(config)#");
}

#[test]
fn copy_dialog_and_startup_mirror() {
    let mut d = router();
    let mut c = Cli::new();
    exec(&mut c, &mut d, "enable");
    // no startup mirror → honest absence line
    let r = exec(&mut c, &mut d, "show startup-config");
    let t = txt(&r.out);
    assert!(t.contains("configuration memory is not present"), "{t}");
    // copy running-config startup-config arms the filename dialog
    let r = exec(&mut c, &mut d, "copy running-config startup-config");
    assert!(r.pending.is_some(), "{}", txt(&r.out));
    // blank line submits the default destination → nvram write
    let r = exec(&mut c, &mut d, "");
    assert!(c.pending.is_none(), "dialog consumed the blank line");
    assert!(
        r.out.iter().any(|o| matches!(o, Op::Intent(Intent::NvramWrite { .. }))),
        "blank answer must submit the [startup-config] default: {}",
        txt(&r.out)
    );
}

#[test]
fn pending_blank_defaults_all_dialogs() {
    let mut d = router();
    let mut c = Cli::new();
    exec(&mut c, &mut d, "enable");

    // write erase: blank Enter = [confirm] yes
    exec(&mut c, &mut d, "write erase");
    assert!(c.pending.is_some());
    let r = exec(&mut c, &mut d, "");
    assert!(c.pending.is_none());
    assert!(
        r.out.iter().any(|o| matches!(o, Op::Intent(Intent::NvramErase))),
        "blank erase answer must fire NvramErase: {}",
        txt(&r.out)
    );

    // reload: blank save leg = yes → write intent + proceed leg armed
    let r = exec(&mut c, &mut d, "reload");
    assert!(r.pending.is_some());
    let r = exec(&mut c, &mut d, "");
    assert!(
        r.out.iter().any(|o| matches!(o, Op::Intent(Intent::NvramWrite { .. }))),
        "blank save=yes must write nvram first: {}",
        txt(&r.out)
    );
    assert!(
        r.pending.is_some() && r.pending.unwrap().prompt.contains("Proceed"),
        "save leg chains into the proceed question"
    );
    // blank proceed leg = yes → reload intent
    let r = exec(&mut c, &mut d, "");
    assert!(c.pending.is_none());
    assert!(
        r.out.iter().any(|o| matches!(o, Op::Intent(Intent::Reload))),
        "blank proceed=yes must fire Reload: {}",
        txt(&r.out)
    );
}

#[test]
fn pending_field_typed_value_overrides_default() {
    let mut d = router();
    let mut c = Cli::new();
    exec(&mut c, &mut d, "enable");
    exec(&mut c, &mut d, "copy running-config startup-config");
    assert!(c.pending.is_some());
    // a typed destination replaces the default and fails honestly
    let r = exec(&mut c, &mut d, "bogus.cfg");
    assert!(c.pending.is_none());
    let t = txt(&r.out);
    assert!(
        t.contains("%Error opening nvram:bogus.cfg"),
        "typed destination must be the submitted value: {t}"
    );
    assert!(
        !r.out.iter().any(|o| matches!(o, Op::Intent(Intent::NvramWrite { .. }))),
        "no write may fire for a bad destination"
    );
    assert!(!r.mutated);
    // typing the default name explicitly also resolves
    exec(&mut c, &mut d, "copy running-config startup-config");
    let r = exec(&mut c, &mut d, "startup-config");
    assert!(
        r.out.iter().any(|o| matches!(o, Op::Intent(Intent::NvramWrite { .. }))),
        "typed startup-config writes nvram: {}",
        txt(&r.out)
    );
}

#[test]
fn script_from_submode_runs_at_exec_and_restores() {
    let mut d = router();
    let mut c = Cli::new();
    exec(&mut c, &mut d, "enable");
    exec(&mut c, &mut d, "conf t");
    exec(&mut c, &mut d, "interface g0/0");
    assert_eq!(c.node, netlab_core::cli::types::ModeId::ConfigIf);
    let mut ev = Vec::new();
    // the prelude is written for the Exec seat — a script issued from a
    // submode must still configure, then restore the caller's seat
    let r = c.script(&mut d, "hostname FromScript\n", 0, &mut ev);
    assert!(r.errors.is_empty(), "{:?}", r.errors);
    assert!(r.mutated);
    assert_eq!(d.name, "FromScript");
    assert_eq!(c.node, netlab_core::cli::types::ModeId::ConfigIf);
    assert_eq!(c.ctx.if_port, Some(0), "config-if context is restored");
    // the echo shows each line ran at the mode the prelude navigated to
    let echoes: Vec<String> = r
        .out
        .iter()
        .filter_map(|o| match o {
            Op::Echo { text } => Some(text.clone()),
            _ => None,
        })
        .collect();
    assert!(
        echoes.iter().any(|e| e.contains("(config)# hostname FromScript")),
        "hostname ran at (config), not the caller's (config-if): {echoes:?}"
    );
}

#[test]
fn no_miss_reports_no_mutation() {
    let mut d = switch();
    let mut c = Cli::new();
    exec(&mut c, &mut d, "enable");
    exec(&mut c, &mut d, "conf t");
    // vlan that was never created — IOS accepts silently, but mutated=false
    let r = exec(&mut c, &mut d, "no vlan 3000");
    assert!(!r.mutated, "no-miss must not report mutation: {}", txt(&r.out));
    assert!(!txt(&r.out).contains("Invalid input"), "{}", txt(&r.out));
    let mut d = router();
    let mut c = Cli::new();
    exec(&mut c, &mut d, "enable");
    exec(&mut c, &mut d, "conf t");
    let r = exec(&mut c, &mut d, "no router ospf 5");
    assert!(!r.mutated, "absent ospf process: {}", txt(&r.out));
    let r = exec(&mut c, &mut d, "no access-list 77");
    assert!(!r.mutated, "absent acl: {}", txt(&r.out));
    // interface-level miss: no description set → no ip access-group
    exec(&mut c, &mut d, "interface g0/0");
    let r = exec(&mut c, &mut d, "no ip access-group 10 in");
    assert!(!r.mutated, "absent access-group: {}", txt(&r.out));
    // a hit still mutates
    exec(&mut c, &mut d, "description wired");
    let r = exec(&mut c, &mut d, "no description");
    assert!(r.mutated, "removing a present description mutates");
    assert!(d.ports[0].description.is_none());
}

#[test]
fn mode_verbs_not_admitted_from_submodes() {
    let mut d = router();
    let mut c = Cli::new();
    exec(&mut c, &mut d, "enable");
    exec(&mut c, &mut d, "conf t");
    exec(&mut c, &mut d, "interface g0/0");
    // `disable` walked up from config-if must not silently leave config
    for verb in ["disable", "enable", "configure terminal"] {
        let r = exec(&mut c, &mut d, verb);
        let t = txt(&r.out);
        assert!(
            t.contains("Invalid input"),
            "{verb} in config-if must error, not silently act: {t}"
        );
        assert_eq!(r.prompt, "R1(config-if)#", "{verb} must not change mode");
    }
}

#[test]
fn hidden_ping_c_parses_not_in_help() {
    let mut d = router();
    let mut c = Cli::new();
    exec(&mut c, &mut d, "enable");
    let r = exec(&mut c, &mut d, "ping -c 5 10.0.0.1");
    let t = txt(&r.out);
    assert!(!t.contains("Invalid input"), "{t}");
    // but `ping ?` must not advertise -c
    let h = c.help(&d, "ping ");
    match h {
        HelpResult::Rows { rows, .. } => {
            assert!(!rows.iter().any(|r| r.word == "-c"), "{rows:?}");
            assert!(rows.iter().any(|r| r.word == "<A.B.C.D>"), "{rows:?}");
        }
        other => panic!("expected rows, got {other:?}"),
    }
}

#[test]
fn host_inventory_dhcp_pcname_trace() {
    let mut d = host();
    let mut c = Cli::new();
    let r = exec(&mut c, &mut d, "ip dhcp");
    assert!(r.mutated, "{}", txt(&r.out));
    let r = exec(&mut c, &mut d, "set pcname WORKSTATION");
    assert!(r.mutated);
    assert_eq!(d.name, "WORKSTATION");
    let r = exec(&mut c, &mut d, "trace 8.8.8.8");
    assert!(r.out.iter().any(|o| matches!(o, Op::Intent(Intent::Trace { .. }))));
    let r = exec(&mut c, &mut d, "arp");
    assert!(!txt(&r.out).contains("Invalid input"), "{}", txt(&r.out));
    let r = exec(&mut c, &mut d, "show arp");
    assert!(!txt(&r.out).contains("Invalid input"), "{}", txt(&r.out));
    // host has no mode transitions even for armed-looking commands
    let r = exec(&mut c, &mut d, "configure terminal");
    assert!(txt(&r.out).contains("Invalid input"));
    assert_eq!(r.prompt, "WORKSTATION>");
}

#[test]
fn default_edge_and_no_variants() {
    let mut d = router();
    let mut c = Cli::new();
    exec(&mut c, &mut d, "enable");
    exec(&mut c, &mut d, "conf t");
    exec(&mut c, &mut d, "hostname Changed");
    let r = exec(&mut c, &mut d, "default hostname");
    // default → restore kind default name
    assert!(r.mutated);
    assert_eq!(d.name, "Router");
    exec(&mut c, &mut d, "interface g0/0");
    exec(&mut c, &mut d, "shutdown");
    assert!(!d.ports[0].admin_up);
    // `default shutdown` on a router restores admin_down (kind default)
    let r = exec(&mut c, &mut d, "default shutdown");
    assert!(r.mutated);
    assert!(!d.ports[0].admin_up);
    exec(&mut c, &mut d, "no shutdown");
    assert!(d.ports[0].admin_up);
}

#[test]
fn neg_route_optional_nh_and_mac_clear() {
    let mut d = router();
    let mut c = Cli::new();
    exec(&mut c, &mut d, "enable");
    exec(&mut c, &mut d, "conf t");
    exec(&mut c, &mut d, "ip route 9.9.9.0 255.255.255.0 10.0.0.254");
    // bare prefix form
    let r = exec(&mut c, &mut d, "no ip route 9.9.9.0 255.255.255.0");
    let t = txt(&r.out);
    assert!(!t.contains("Incomplete"), "{t}");
    // with next-hop
    exec(&mut c, &mut d, "ip route 9.9.9.0 255.255.255.0 10.0.0.254");
    let r = exec(&mut c, &mut d, "no ip route 9.9.9.0 255.255.255.0 10.0.0.254");
    let t = txt(&r.out);
    assert!(!t.contains("Invalid input"), "{t}");
    // switch mac table clear
    let mut s = switch();
    let mut cs = Cli::new();
    exec(&mut cs, &mut s, "enable");
    let r = exec(&mut cs, &mut s, "clear mac address-table");
    assert!(!txt(&r.out).contains("Invalid input"), "{}", txt(&r.out));
    // the hyphenated alias is not in the grammar → honest invalid
    let r = exec(&mut cs, &mut s, "clear mac-address-table");
    assert!(txt(&r.out).contains("Invalid input"), "{}", txt(&r.out));
    // vlan lists beyond the expansion cap are honest errors, not bombs
    exec(&mut cs, &mut s, "conf t");
    exec(&mut cs, &mut s, "interface fa0/1");
    exec(&mut cs, &mut s, "switchport mode trunk");
    let r = exec(&mut cs, &mut s, "switchport trunk allowed vlan 1-200");
    assert!(r.mutated, "{}", txt(&r.out));
    let r = exec(&mut cs, &mut s, "switchport trunk allowed vlan 1-500");
    assert!(txt(&r.out).contains("Invalid input"), "{}", txt(&r.out));
    assert!(!r.mutated);
}

#[test]
fn vlan_id_boundaries_at_the_grammar() {
    let mut d = switch();
    let mut c = Cli::new();
    exec(&mut c, &mut d, "enable");
    exec(&mut c, &mut d, "conf t");
    for line in ["vlan 0", "vlan 4095"] {
        let r = exec(&mut c, &mut d, line);
        let t = txt(&r.out);
        assert!(t.contains("Invalid input"), "{line}: {t}");
        assert!(!r.mutated, "{line} must not mutate");
        assert_eq!(r.prompt, "S1(config)#");
    }
    assert!(!d.vlans.contains_key(&0) && !d.vlans.contains_key(&4095));
    let r = exec(&mut c, &mut d, "vlan 4094");
    assert_eq!(r.prompt, "S1(config-vlan)#");
    assert!(d.vlans.contains_key(&4094));
}

#[test]
fn whitespace_only_lines_are_blank_input() {
    let mut d = router();
    let mut c = Cli::new();
    exec(&mut c, &mut d, "enable");
    let before = c.history.len();
    for line in ["   ", "\t ", "\u{00a0}"] {
        let r = exec(&mut c, &mut d, line);
        assert!(r.out.is_empty(), "{line:?} must produce no output");
        assert!(!r.mutated, "{line:?} must not mutate");
        assert_eq!(r.prompt, "R1#");
    }
    assert_eq!(c.history.len(), before, "blank lines never enter history");
}

#[test]
fn invisible_format_chars_rejected_in_names() {
    let mut d = router();
    let mut c = Cli::new();
    exec(&mut c, &mut d, "enable");
    exec(&mut c, &mut d, "conf t");
    for bad in ["a\u{202e}b", "a\u{2028}b", "a\u{200b}b", "a\u{feff}b"] {
        let r = exec(&mut c, &mut d, &format!("hostname {bad}"));
        let t = txt(&r.out);
        assert!(t.contains("Illegal characters"), "{bad:?}: {t}");
        assert!(!r.mutated, "{bad:?} must not mutate");
    }
    assert_eq!(d.name, "R1");
}

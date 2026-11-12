// Fixture table + rig builders for the spy harness — a port of
// core/tests/cli/fixtures.rs expressed through the wasm export surface.
// corpus-steps.json carries no header fields, so each record's kind/topo/
// tick/ifmap/startup lives here; a drifted table fails the byte-compare
// loudly. Seeds go through lab setters + apply_device_fields, never
// through cli_script.
//
// Ordering mirrors the observable state the fixtures leave:
//   1. pump(tick) on the unwired rig — the runner assigns `topo.tick`
//      directly; the wasm surface has no setter, but step() on a rig
//      with no links/services/ospf is inert, so pumping before wiring
//      leaves the same counter with zero side effects.
//   2. seeds — ip + admin through apply_device_fields (silent by
//      contract: authored seats never fabricate events).
//   3. wire — add_link_ports refreshes carrier on both ends and emits
//      the C1/C2 port-state pair into the queue + buffered log, the
//      same content set_admin emits in the fixtures.
//   4. seedLate — ospf_enter derives router_id from carrier, so the
//      OSPF seat must land after the wire.
//   5. drain the setup queue (buffered log retains the lines).

const seats = fields => JSON.stringify(fields);
const adminUp = (lab, dev, port) =>
  lab.apply_device_fields(dev, seats({ ports: [{ index: port, admin_up: true }] }));

const singleRouter = lab => ({
  devs: [{ id: lab.add_device('router', 'R', 0, 0), kind: 'router', name: 'R' }],
  seeds() {},
  wire() {},
});

const singleSwitch = lab => ({
  devs: [{ id: lab.add_device('switch', 'SW', 0, 0), kind: 'switch', name: 'SW' }],
  seeds() {},
  wire() {},
});

const twoRouterLink = lab => {
  const r1 = lab.add_device('router', 'R1', 0, 0);
  const r2 = lab.add_device('router', 'R2', 100, 0);
  return {
    devs: [
      { id: r1, kind: 'router', name: 'R1' },
      { id: r2, kind: 'router', name: 'R2' },
    ],
    seeds() {
      lab.set_ip(r2, 0, '10.0.0.2', '255.255.255.0');
      adminUp(lab, r2, 0);
    },
    wire() {
      lab.add_link_ports(r1, 0, r2, 0);
    },
  };
};

const switchHosts = lab => {
  const sw = lab.add_device('switch', 'SW', 0, 0);
  const h1 = lab.add_device('host', 'PC1', 100, 0);
  const h2 = lab.add_device('host', 'PC2', 200, 0);
  return {
    devs: [
      { id: sw, kind: 'switch', name: 'SW' },
      { id: h1, kind: 'host', name: 'PC1' },
      { id: h2, kind: 'host', name: 'PC2' },
    ],
    seeds() {},
    wire() {
      lab.add_link_ports(h1, 0, sw, 0);
      lab.add_link_ports(h2, 0, sw, 1);
    },
    seedLate() {
      lab.apply_device_fields(sw, seats({ vlans: { '10': { name: 'blue' } } }));
      lab.set_switchport(sw, 2, 'access', 10, 0, '');
    },
  };
};

const ospfPair = lab => {
  const r1 = lab.add_device('router', 'R1', 0, 0);
  const r2 = lab.add_device('router', 'R2', 100, 0);
  const ospf = { pid: 1, networks: [{ net: '10.0.0.0', wild: '0.0.0.255', area: 0 }] };
  return {
    devs: [
      { id: r1, kind: 'router', name: 'R1' },
      { id: r2, kind: 'router', name: 'R2' },
    ],
    seeds() {
      lab.set_ip(r1, 0, '10.0.0.1', '255.255.255.0');
      adminUp(lab, r1, 0);
      lab.set_ip(r2, 0, '10.0.0.2', '255.255.255.0');
      adminUp(lab, r2, 0);
    },
    wire() {
      lab.add_link_ports(r1, 0, r2, 0);
    },
    seedLate() {
      lab.apply_device_fields(r1, seats({ ospf }));
      lab.apply_device_fields(r2, seats({ ospf }));
      lab.add_route(r1, '10.9.0.0', '255.255.255.0', '10.0.0.2', 0);
    },
  };
};

const natDhcp = lab => {
  const r = lab.add_device('router', 'R', 0, 0);
  const h = lab.add_device('host', 'PC', 100, 0);
  return {
    devs: [
      { id: r, kind: 'router', name: 'R' },
      { id: h, kind: 'host', name: 'PC' },
    ],
    seeds() {
      lab.set_ip(r, 0, '192.168.1.1', '255.255.255.0');
      adminUp(lab, r, 0);
      lab.set_nat(r, 0, 'inside');
      lab.set_ip(r, 1, '203.0.113.1', '255.255.255.0');
      adminUp(lab, r, 1);
      lab.set_nat(r, 1, 'outside');
      lab.set_services(r, true, false, false);
      // fixture seeds dhcp_excluded_add(192.168.1.1); no wasm seat reaches
      // it — unobservable here: the default pool starts at net+10 and
      // alloc_lease skips interface IPs.
    },
    wire() {
      lab.add_link_ports(h, 0, r, 0);
    },
  };
};

const TOPOS = {
  'single-router': singleRouter,
  'single-switch': singleSwitch,
  'two-router-link': twoRouterLink,
  'switch-hosts': switchHosts,
  'ospf-pair': ospfPair,
  'nat-dhcp': natDhcp,
};

export const RECORD_RIGS = {
  'c01-modes-ladder': { kind: 'router', topo: 'single-router' },
  'c02-triad-invalid': { kind: 'router', topo: 'single-router' },
  'c03-triad-incomplete': { kind: 'router', topo: 'single-router' },
  'c04-triad-ambiguous': { kind: 'router', topo: 'single-router' },
  'c05-abbrev-ifname': {
    kind: 'router',
    topo: 'single-router',
    ifnames: ['g0/0', 'g0/1', 'g0/2', 'g0/3', 'g0/4', 'g0/5', 'g0/6', 'g0/7'],
  },
  'c06-pending-dialogs': { kind: 'router', topo: 'single-router' },
  'c07-pending-field': { kind: 'router', topo: 'single-router' },
  'c08-no-forms': { kind: 'router', topo: 'single-router' },
  'c09-kind-gates': { kind: 'switch', topo: 'single-switch' },
  'c10-show-bank-router': { kind: 'router', topo: 'ospf-pair', tick: 100 },
  'c11-show-bank-switch': { kind: 'switch', topo: 'switch-hosts' },
  'c12-script-flat': { kind: 'router', topo: 'single-router' },
  'c13-script-prelude-dedup': { kind: 'router', topo: 'single-router' },
  'c14-script-mode-restore': { kind: 'router', topo: 'single-router' },
  'c15-startup-mirror': {
    kind: 'router',
    topo: 'single-router',
    startup: 'hostname Saved\ninterface GigabitEthernet0/0\n ip address 10.5.5.5 255.255.255.0',
  },
  'c16-help-complete': { kind: 'switch', topo: 'single-switch' },
  'c17-events-pinned': { kind: 'host', topo: 'nat-dhcp' },
  'c18-nvgen-seeds': { kind: 'router', topo: 'ospf-pair' },
  'c19-gap-ledger': { kind: 'router', topo: 'single-router' },
  'c20-history-clear': { kind: 'router', topo: 'two-router-link' },
};

export function buildRig(lab, rig) {
  const builder = TOPOS[rig.topo];
  if (!builder) throw new Error(`unknown topo ${rig.topo}`);
  const built = builder(lab);
  for (let i = 0; i < (rig.tick ?? 0); i++) lab.tick();
  built.seeds();
  built.wire();
  built.seedLate?.();
  lab.drain_events();
  for (const d of built.devs) lab.baseline_commit(d.id);
  const row = built.devs.find(d => d.kind === rig.kind);
  if (!row) throw new Error(`topo ${rig.topo} has no device of kind ${rig.kind}`);
  if (rig.ifnames) lab.cli_set_ifnames(row.id, rig.ifnames);
  lab.cli_set_startup(row.id, rig.startup ?? null);
  return { dev: row.id, devs: built.devs };
}

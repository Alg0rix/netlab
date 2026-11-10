// Curated topology template marketplace (Beginner -> Expert) + user-saved templates.

const USER_TEMPLATES_KEY = 'netlab.user_templates.v1';

const lenMask = n => [0, 1, 2, 3].map(i => {
  const b = Math.max(0, Math.min(8, n - i * 8));
  return (0xff << (8 - b)) & 0xff;
}).join('.');

const N = (id, tpl, name, x, y, ports = [], routes = [], extra = {}) => ({
  id, tpl, name, x, y, ports, routes, ...extra,
});
const ip = (a, len = 24) => (a ? { ip: a, mask: lenMask(len) } : null);
const gw = (g, out_port = 0) => [{ prefix: '0.0.0.0', mask: '0.0.0.0', next_hop: g, out_port }];
const rt = (prefix, len, next_hop, out_port = 0) => ({ prefix, mask: lenMask(len), next_hop, out_port });
const L = (a, ap, b, bp, up = true) => ({ a, ap, b, bp, up });
const S = (id, kind, x, y, w, h, stroke, text = '', fill = true, dash = false) => ({
  id, kind, x, y, w, h, stroke, fill, dash, width: 2, text,
});

export const LEVEL_META = {
  beginner:     { label: 'Beginner',     color: 'ok',     order: 1 },
  intermediate: { label: 'Intermediate', color: 'accent', order: 2 },
  advanced:     { label: 'Advanced',     color: 'warn',   order: 3 },
  expert:       { label: 'Expert',       color: 'violet', order: 4 },
  custom:       { label: 'My Template',  color: 'teal',   order: 5 },
};

export const BUILTIN_TEMPLATES = [
  /* ────────────────────────────── BEGINNER ────────────────────────────── */
  {
    id: 'p2p-basics',
    title: 'Point-to-Point Ethernet Basics',
    level: 'beginner',
    category: 'Foundations',
    summary: 'Two PCs connected by a single Ethernet patch cord. The simplest lab to watch ARP resolution and ICMP echo packets frame by frame.',
    tags: ['ARP', 'ICMP', 'Ethernet', 'Packet Capture'],
    objectives: [
      'Double-click PC1 and run `ping 10.0.0.2` to watch ARP and ICMP exchange.',
      'Right-click the link and choose Start capture to inspect Ethernet II, ARP, and ICMP headers.',
      'Switch to Simulation mode (top bar) and press `.` to step one tick at a time.',
    ],
    build: () => ({
      format: 'netlab', version: 1, name: 'Point-to-Point Basics',
      nodes: [
        N(1, 'pc', 'PC1', 260, 260, [ip('10.0.0.1', 24)]),
        N(2, 'pc', 'PC2', 620, 260, [ip('10.0.0.2', 24)]),
      ],
      links: [
        L(1, 0, 2, 0),
      ],
      shapes: [
        S(1, 'rect', 180, 170, 520, 190, '#4c9bff', '10.0.0.0/24 · Point-to-Point Link'),
      ],
      notes: [
        { id: 1, x: 180, y: 60, text: 'Lab 01 · Point-to-Point Ethernet\n1. Double-click PC1 → ping 10.0.0.2\n2. Double-click the cable to capture frames\n3. Run "show arp" on PC1 to see learned MAC' },
      ],
    }),
  },

  {
    id: 'switched-lan',
    title: 'Switched LAN & MAC Learning',
    level: 'beginner',
    category: 'Switching',
    summary: 'Four endpoints sharing a single Layer-2 Ethernet switch. Explore broadcast ARP flooding vs. unicast forwarding and MAC table learning.',
    tags: ['L2 Switch', 'MAC Table', 'Broadcast Domain', 'LAN'],
    objectives: [
      'Switch to Simulation mode and run `ping 192.168.1.20` from PC1.',
      'Observe how the first ARP Request floods out all switch ports, while the reply and subsequent pings travel unicast.',
      'Select SW1 to inspect its dynamically learned MAC address table.',
    ],
    build: () => ({
      format: 'netlab', version: 1, name: 'Switched Office LAN',
      nodes: [
        N(1, 'switch', 'SW1', 440, 250),
        N(2, 'pc', 'PC1', 200, 130, [ip('192.168.1.11', 24)]),
        N(3, 'pc', 'PC2', 200, 370, [ip('192.168.1.12', 24)]),
        N(4, 'laptop', 'Laptop1', 680, 130, [ip('192.168.1.13', 24)]),
        N(5, 'server', 'FileSrv', 680, 370, [ip('192.168.1.20', 24)]),
      ],
      links: [
        L(1, 0, 2, 0),
        L(1, 1, 3, 0),
        L(1, 2, 4, 0),
        L(1, 3, 5, 0),
      ],
      shapes: [
        S(1, 'rect', 120, 60, 640, 390, '#3ccf7e', 'Broadcast Domain · 192.168.1.0/24'),
      ],
      notes: [
        { id: 1, x: 120, y: 470, text: 'Lab 02 · Switched LAN\n• Ping FileSrv (192.168.1.20) from PC1\n• Open SW1 console → show mac-address-table\n• Notice how SW1 learns source MACs on e0..e3' },
      ],
    }),
  },

  {
    id: 'single-router-lan',
    title: 'Two Subnets, One Router',
    level: 'beginner',
    category: 'Routing',
    summary: 'Classic inter-subnet routing across a single router connecting Sales (192.168.10.0/24) and Engineering (192.168.20.0/24) LANs.',
    tags: ['Default Gateway', 'IPv4 Subnets', 'Connected Routes', 'TTL'],
    objectives: [
      'Ping from PC-Sales1 (192.168.10.11) to Web-Eng (192.168.20.20).',
      'Compare Ethernet source/destination MAC addresses on both sides of R1 using packet capture.',
      'Try clearing the default gateway on PC-Sales2 (`clear gateway`) and observe why off-subnet pings fail.',
    ],
    build: () => ({
      format: 'netlab', version: 1, name: 'Two Subnets One Router',
      nodes: [
        N(1, 'router', 'R1', 460, 120, [ip('192.168.10.1', 24), ip('192.168.20.1', 24)]),
        N(2, 'switch', 'SW-Sales', 240, 270),
        N(3, 'switch', 'SW-Eng', 680, 270),
        N(4, 'pc', 'PC-Sales1', 130, 420, [ip('192.168.10.11', 24)], gw('192.168.10.1')),
        N(5, 'laptop', 'PC-Sales2', 310, 420, [ip('192.168.10.12', 24)], gw('192.168.10.1')),
        N(6, 'pc', 'PC-Eng1', 590, 420, [ip('192.168.20.11', 24)], gw('192.168.20.1')),
        N(7, 'server', 'Web-Eng', 770, 420, [ip('192.168.20.20', 24)], gw('192.168.20.1')),
      ],
      links: [
        L(1, 0, 2, 0),
        L(1, 1, 3, 0),
        L(2, 1, 4, 0),
        L(2, 2, 5, 0),
        L(3, 1, 6, 0),
        L(3, 2, 7, 0),
      ],
      shapes: [
        S(1, 'rect', 70, 210, 310, 280, '#4c9bff', 'Sales LAN · 192.168.10.0/24'),
        S(2, 'rect', 530, 210, 310, 280, '#3ccf7e', 'Engineering LAN · 192.168.20.0/24'),
      ],
      notes: [
        { id: 1, x: 70, y: 50, text: 'Lab 03 · Inter-Subnet Routing\n• R1 routes between g0/0 (.10.1) and g0/1 (.20.1)\n• Double-click PC-Sales1 → ping 192.168.20.20' },
      ],
    }),
  },

  {
    id: 'daisy-chain-switches',
    title: 'Cascaded Switches & Inter-Switch Trunking',
    level: 'beginner',
    category: 'Switching',
    summary: 'Two access switches linked by an inter-switch trunk across two floors sharing a single flat /24 broadcast domain.',
    tags: ['Inter-Switch Link', 'MAC Flooding', 'L2 Trunk', 'Flat LAN'],
    objectives: [
      'Ping from `Floor1-PC1` (`10.10.0.11`) to `Floor2-Srv` (`10.10.0.22`) across the `SW-Floor1 ↔ SW-Floor2` uplink.',
      'Inspect `show mac-address-table` on both switches and notice how `SW-Floor1` learns all Floor-2 MACs on its uplink port `e0`.',
      'Start a packet capture on the inter-switch link (`SW-Floor1 e0 ↔ SW-Floor2 e0`) to watch ARP broadcasts cross between closets.',
    ],
    build: () => ({
      format: 'netlab', version: 1, name: 'Cascaded Switches',
      nodes: [
        N(1, 'switch', 'SW-Floor1', 280, 240),
        N(2, 'switch', 'SW-Floor2', 640, 240),
        N(3, 'pc', 'Floor1-PC1', 120, 120, [ip('10.10.0.11', 24)]),
        N(4, 'pc', 'Floor1-PC2', 120, 240, [ip('10.10.0.12', 24)]),
        N(5, 'laptop', 'Floor1-Lap', 120, 360, [ip('10.10.0.13', 24)]),
        N(6, 'pc', 'Floor2-PC1', 800, 120, [ip('10.10.0.21', 24)]),
        N(7, 'server', 'Floor2-Srv', 800, 240, [ip('10.10.0.22', 24)]),
        N(8, 'laptop', 'Floor2-Lap', 800, 360, [ip('10.10.0.23', 24)]),
      ],
      links: [
        L(1, 0, 2, 0),
        L(1, 1, 3, 0), L(1, 2, 4, 0), L(1, 3, 5, 0),
        L(2, 1, 6, 0), L(2, 2, 7, 0), L(2, 3, 8, 0),
      ],
      shapes: [
        S(1, 'rect', 50, 60, 320, 370, '#4c9bff', 'Floor 1 Closet · 10.10.0.0/24'),
        S(2, 'rect', 550, 60, 320, 370, '#3ccf7e', 'Floor 2 Closet · 10.10.0.0/24'),
      ],
      notes: [
        { id: 1, x: 340, y: 460, text: 'Cascaded Switches Lab\n• SW-Floor1 e0 ↔ SW-Floor2 e0 carries all inter-floor frames\n• Ping 10.10.0.22 from Floor1-PC1 and inspect both MAC tables' },
      ],
    }),
  },

  {
    id: 'soho-home-network',
    title: 'Home / SOHO Gateway & ISP Uplink',
    level: 'beginner',
    category: 'Foundations',
    summary: 'Everyday home broadband topology: Desktop, Laptop, and NAS behind a Home Router connected to an ISP Edge router and public Cloud Server.',
    tags: ['SOHO', 'Home Gateway', 'ISP Uplink', 'Traceroute'],
    objectives: [
      'Ping `Home-NAS` (`192.168.1.50`) from `Gaming-PC` (`192.168.1.10`) — traffic stays purely on `SW-Home` at Layer 2.',
      'Run `traceroute 93.184.216.34` from `Gaming-PC` to trace hops through `Home-GW` (`192.168.1.1`) and `ISP-Edge` (`100.64.0.1`).',
      'Capture on the WAN link (`100.64.0.0/30`) to compare LAN vs. WAN packet headers.',
    ],
    build: () => ({
      format: 'netlab', version: 1, name: 'SOHO Home Network',
      nodes: [
        N(1, 'switch', 'SW-Home', 240, 280),
        N(2, 'router', 'Home-GW', 450, 280, [
          ip('192.168.1.1', 24), ip('100.64.0.2', 30),
        ], [
          rt('0.0.0.0', 0, '100.64.0.1', 1),
        ]),
        N(3, 'router', 'ISP-Edge', 660, 280, [
          ip('100.64.0.1', 30), ip('93.184.216.1', 24),
        ], [
          rt('192.168.1.0', 24, '100.64.0.2', 0),
        ]),
        N(4, 'pc', 'Gaming-PC', 90, 150, [ip('192.168.1.10', 24)], gw('192.168.1.1')),
        N(5, 'laptop', 'Work-Laptop', 90, 280, [ip('192.168.1.11', 24)], gw('192.168.1.1')),
        N(6, 'server', 'Home-NAS', 90, 410, [ip('192.168.1.50', 24)], gw('192.168.1.1')),
        N(7, 'server', 'Cloud-Web', 840, 280, [ip('93.184.216.34', 24)], gw('93.184.216.1')),
      ],
      links: [
        L(1, 0, 2, 0),
        L(2, 1, 3, 0),
        L(3, 1, 7, 0),
        L(1, 1, 4, 0), L(1, 2, 5, 0), L(1, 3, 6, 0),
      ],
      shapes: [
        S(1, 'rect', 30, 90, 490, 390, '#3ccf7e', 'Home LAN · 192.168.1.0/24'),
        S(2, 'rect', 590, 170, 320, 220, '#4c9bff', 'ISP & Public Internet · 93.184.216.0/24'),
      ],
      notes: [
        { id: 1, x: 560, y: 70, text: 'SOHO Broadband Lab\n• LAN: 192.168.1.0/24\n• WAN Link: 100.64.0.0/30 (CGNAT range)\n• Run "traceroute 93.184.216.34" on Gaming-PC' },
      ],
    }),
  },

  {
    id: 'subnet-mask-lab',
    title: 'CIDR Subnetting Practice (/25, /26 & /30)',
    level: 'beginner',
    category: 'Foundations',
    summary: 'Hands-on Variable Length Subnet Mask (VLSM) lab splitting 10.50.0.0 into /25 halves, a /26 server block, and a /30 point-to-point link.',
    tags: ['CIDR', 'VLSM', 'Subnet Masks', '/25 & /26'],
    objectives: [
      'Notice that `Host-Low` (`10.50.0.10/25`) and `Host-High` (`10.50.0.140/25`) look like they share `10.50.0.x`, but `.128` splits them into two distinct `/25` subnets!',
      'Ping `Host-High` (`10.50.0.140`) from `Host-Low` (`10.50.0.10`) and verify it routes through `R-VLSM` (`10.50.0.1` → `10.50.0.129`).',
      'Inspect `show ip interface` on `R-VLSM` to compare `/25` (`255.255.255.128`), `/26` (`255.255.255.192`), and `/30` (`255.255.255.252`).',
    ],
    build: () => ({
      format: 'netlab', version: 1, name: 'CIDR & VLSM Subnetting',
      nodes: [
        N(1, 'router', 'R-VLSM', 460, 250, [
          ip('10.50.0.1', 25), ip('10.50.0.129', 25), ip('10.50.1.1', 26), ip('10.50.255.1', 30),
        ]),
        N(2, 'pc', 'Host-Low1', 160, 130, [ip('10.50.0.10', 25)], gw('10.50.0.1')),
        N(3, 'pc', 'Host-High1', 760, 130, [ip('10.50.0.140', 25)], gw('10.50.0.129')),
        N(4, 'server', 'Srv-Block', 160, 380, [ip('10.50.1.10', 26)], gw('10.50.1.1')),
        N(5, 'router', 'R-Peer', 760, 380, [ip('10.50.255.2', 30)], [
          rt('0.0.0.0', 0, '10.50.255.1', 0),
        ]),
      ],
      links: [
        L(1, 0, 2, 0),
        L(1, 1, 3, 0),
        L(1, 2, 4, 0),
        L(1, 3, 5, 0),
      ],
      shapes: [
        S(1, 'rect', 70, 65, 220, 130, '#4c9bff', '10.50.0.0/25 (.1 - .126)'),
        S(2, 'rect', 650, 65, 240, 130, '#3ccf7e', '10.50.0.128/25 (.129 - .254)'),
        S(3, 'rect', 70, 315, 220, 130, '#f2b84b', '10.50.1.0/26 (62 hosts)'),
        S(4, 'rect', 650, 315, 240, 130, '#a58bff', '10.50.255.0/30 (2 P2P IPs)'),
      ],
      notes: [
        { id: 1, x: 330, y: 50, text: 'VLSM Subnetting Cheat Sheet\n• /25 = 255.255.255.128 (126 usable hosts)\n• /26 = 255.255.255.192 (62 usable hosts)\n• /30 = 255.255.255.252 (2 usable P2P IPs)' },
      ],
    }),
  },

  /* ──────────────────────────── INTERMEDIATE ──────────────────────────── */
  {
    id: 'branch-office',
    title: 'Branch Office & Cisco IOS Edge',
    level: 'intermediate',
    category: 'Enterprise',
    summary: 'Default netlab showcase: two switched user/server subnets routed by R1 with an uplink to a real Cisco 7200 IOS router.',
    tags: ['Branch LAN', 'Cisco IOS', 'Hybrid Sim', 'Wireshark PCAP'],
    objectives: [
      'Ping across subnets from PC1 (192.168.10.11) to PC3 (192.168.20.13).',
      'Attach a Cisco 7200 `.bin` image to IOS1 and configure `f0/0` as `10.0.0.2/30` to peer with R1.',
      'Capture packets on the trunk link and export a `.pcap` file.',
    ],
    build: () => ({
      format: 'netlab', version: 1, name: 'Branch office',
      nodes: [
        N(1, 'router', 'R1', 460, 120, [ip('192.168.10.1'), ip('192.168.20.1'), ip('10.0.0.1', 30)]),
        N(2, 'switch', 'SW1', 280, 280),
        N(3, 'switch', 'SW2', 640, 280),
        N(4, 'pc', 'PC1', 140, 440, [ip('192.168.10.11')], gw('192.168.10.1')),
        N(5, 'pc', 'PC2', 280, 440, [ip('192.168.10.12')], gw('192.168.10.1')),
        N(6, 'server', 'Web1', 420, 440, [ip('192.168.10.20')], gw('192.168.10.1')),
        N(7, 'pc', 'PC3', 580, 440, [ip('192.168.20.13')], gw('192.168.20.1')),
        N(8, 'laptop', 'Laptop1', 720, 440, [ip('192.168.20.31')], gw('192.168.20.1')),
        N(9, 'ios', 'IOS1', 780, 120),
      ],
      links: [
        L(1, 0, 2, 0), L(1, 1, 3, 0), L(1, 2, 9, 0),
        L(2, 1, 4, 0), L(2, 2, 5, 0), L(2, 3, 6, 0),
        L(3, 1, 7, 0), L(3, 2, 8, 0),
      ],
      notes: [
        { id: 1, x: 40, y: 40, text: 'Branch office lab\nDouble-click PC1 → ping 192.168.20.13\nRight-click a link → Start capture' },
      ],
    }),
  },

  {
    id: 'multi-hop-wan',
    title: 'Multi-Site WAN & Static Routing Triangle',
    level: 'intermediate',
    category: 'Routing',
    summary: 'Three regional sites (West, Core, East) linked by /30 WAN point-to-point links with primary and backup paths.',
    tags: ['Static Routes', '/30 WAN Links', 'Multi-Hop', 'Failover'],
    objectives: [
      'Ping from PC-West (172.16.1.10) to Srv-East (172.16.3.20) and inspect `show ip route` on R-West.',
      'Unplug the direct R-West ↔ R-East link (10.0.13.0/30) and update static routes to transit via R-Core (10.0.12.2 / 10.0.23.2).',
      'Watch packets traverse multiple L3 hops in Simulation mode.',
    ],
    build: () => ({
      format: 'netlab', version: 1, name: 'Multi-Site Static Routing',
      nodes: [
        N(1, 'router', 'R-Core', 460, 100, [
          ip('10.0.12.2', 30), ip('10.0.23.1', 30), ip('172.16.2.1', 24),
        ], [
          rt('172.16.1.0', 24, '10.0.12.1', 0),
          rt('172.16.3.0', 24, '10.0.23.2', 1),
        ]),
        N(2, 'router', 'R-West', 220, 260, [
          ip('10.0.12.1', 30), ip('10.0.13.1', 30), ip('172.16.1.1', 24),
        ], [
          rt('172.16.2.0', 24, '10.0.12.2', 0),
          rt('172.16.3.0', 24, '10.0.13.2', 1),
        ]),
        N(3, 'router', 'R-East', 700, 260, [
          ip('10.0.23.2', 30), ip('10.0.13.2', 30), ip('172.16.3.1', 24),
        ], [
          rt('172.16.2.0', 24, '10.0.23.1', 0),
          rt('172.16.1.0', 24, '10.0.13.1', 1),
        ]),
        N(4, 'switch', 'SW-West', 160, 410),
        N(5, 'switch', 'SW-East', 760, 410),
        N(6, 'pc', 'PC-West', 100, 530, [ip('172.16.1.10', 24)], gw('172.16.1.1')),
        N(7, 'laptop', 'Ops-West', 230, 530, [ip('172.16.1.11', 24)], gw('172.16.1.1')),
        N(8, 'server', 'Core-DNS', 460, 230, [ip('172.16.2.53', 24)], gw('172.16.2.1')),
        N(9, 'server', 'Srv-East', 690, 530, [ip('172.16.3.20', 24)], gw('172.16.3.1')),
        N(10, 'pc', 'PC-East', 830, 530, [ip('172.16.3.10', 24)], gw('172.16.3.1')),
      ],
      links: [
        L(2, 0, 1, 0),
        L(1, 1, 3, 0),
        L(2, 1, 3, 1),
        L(1, 2, 8, 0),
        L(2, 2, 4, 0),
        L(4, 1, 6, 0),
        L(4, 2, 7, 0),
        L(3, 2, 5, 0),
        L(5, 1, 9, 0),
        L(5, 2, 10, 0),
      ],
      shapes: [
        S(1, 'rect', 50, 360, 240, 230, '#4c9bff', 'West Site · 172.16.1.0/24'),
        S(2, 'rect', 640, 360, 250, 230, '#3ccf7e', 'East Site · 172.16.3.0/24'),
      ],
      notes: [
        { id: 1, x: 50, y: 50, text: 'Lab 05 · Static Routing Triangle\n• Direct link: R-West ↔ R-East (10.0.13.0/30)\n• Transit via R-Core: 10.0.12.0/30 & 10.0.23.0/30\n• Try unplugging R-West g0/1 ↔ R-East g0/1 and rerouting via R-Core!' },
      ],
    }),
  },

  {
    id: 'multi-homed-dmz',
    title: 'Multi-NIC Bastion & Segmented DMZ',
    level: 'intermediate',
    category: 'Security & Multi-NIC',
    summary: 'Showcases multi-Ethernet hosts and servers bridging isolated DMZ, Corporate, and Out-of-Band Management networks.',
    tags: ['Multi-NIC', 'DMZ', 'Bastion Host', 'OOB Mgmt'],
    objectives: [
      'Inspect `Bastion` (2 NICs) and `AppSrv` (3 NICs) — each host has interfaces in multiple subnets simultaneously.',
      'Open `AppSrv` console and run `show ip` and `show interface` to verify all 3 NICs (`eth0`, `eth1`, `eth2`).',
      'Ping `AppSrv` on its DMZ IP (`10.10.0.20`) from `Corp-PC1`, and on its Backend DB IP (`10.99.0.10`) from `DB-Primary` without routing through `R-Perimeter`.',
    ],
    build: () => ({
      format: 'netlab', version: 1, name: 'Multi-NIC DMZ & Bastion',
      nodes: [
        N(1, 'router', 'R-Perimeter', 360, 100, [
          ip('172.16.10.1', 24), ip('10.10.0.1', 24), ip('192.168.99.1', 24),
        ]),
        N(2, 'switch', 'SW-Corp', 150, 250),
        N(3, 'switch', 'SW-DMZ', 360, 250),
        N(4, 'switch', 'SW-Mgmt', 620, 250),
        N(5, 'switch', 'SW-Backend', 360, 500),
        N(6, 'pc', 'Corp-PC1', 100, 400, [ip('172.16.10.11', 24)], gw('172.16.10.1')),
        N(7, 'laptop', 'Admin-Lap', 740, 400, [ip('192.168.99.10', 24), ip('172.16.10.50', 24)], gw('192.168.99.1', 0), { portCount: 2 }),
        N(8, 'server', 'Bastion', 520, 390, [ip('10.10.0.50', 24), ip('192.168.99.50', 24)], gw('10.10.0.1', 0), { portCount: 2 }),
        N(9, 'server', 'AppSrv', 360, 390, [
          ip('10.10.0.20', 24), ip('10.99.0.10', 24), ip('192.168.99.20', 24),
        ], gw('10.10.0.1', 0), { portCount: 3 }),
        N(10, 'server', 'DB-Primary', 220, 560, [ip('10.99.0.20', 24)], [], { portCount: 2 }),
        N(11, 'server', 'DB-Replica', 500, 560, [ip('10.99.0.21', 24)], [], { portCount: 2 }),
      ],
      links: [
        L(1, 0, 2, 0),
        L(1, 1, 3, 0),
        L(1, 2, 4, 0),
        L(2, 1, 6, 0),
        L(2, 2, 7, 1),
        L(3, 1, 9, 0),
        L(3, 2, 8, 0),
        L(4, 1, 8, 1),
        L(4, 2, 9, 2),
        L(4, 3, 7, 0),
        L(9, 1, 5, 0),
        L(5, 1, 10, 0),
        L(5, 2, 11, 0),
        L(10, 1, 11, 1),
      ],
      shapes: [
        S(1, 'rect', 50, 200, 190, 270, '#4c9bff', 'Corporate · 172.16.10.0/24'),
        S(2, 'rect', 270, 200, 310, 255, '#f2b84b', 'DMZ · 10.10.0.0/24'),
        S(3, 'rect', 590, 200, 220, 270, '#2fd0c0', 'OOB Mgmt · 192.168.99.0/24'),
        S(4, 'rect', 160, 465, 410, 150, '#a58bff', 'Private DB Tier · 10.99.0.0/24 (Air-gapped)'),
      ],
      notes: [
        { id: 1, x: 50, y: 40, text: 'Lab 06 · Multi-NIC Architecture\n• AppSrv has 3 NICs: eth0 (DMZ), eth1 (Private DB), eth2 (OOB Mgmt)\n• DB-Primary & DB-Replica have no default gateway and a direct replication link (eth1)\n• Double-click AppSrv → ping 10.99.0.20 (DB) and ping 172.16.10.11 (Corp)' },
      ],
    }),
  },

  {
    id: 'hub-and-spoke-wan',
    title: 'Hub-and-Spoke Enterprise WAN (HQ + 3 Branches)',
    level: 'intermediate',
    category: 'WAN & Routing',
    summary: 'Central HQ Hub router aggregating three regional spoke branches (New York, London, Tokyo) over dedicated /30 WAN serial/point-to-point uplinks.',
    tags: ['Hub-and-Spoke', 'Default Route', 'Branch WAN', 'Route Aggregation'],
    objectives: [
      'Notice how each Spoke router (`R-NY`, `R-LDN`, `R-TYO`) only needs a single `0.0.0.0/0` default route pointing to `R-HQ-Hub`, whereas `R-HQ-Hub` holds explicit `/24` routes for every branch.',
      'Run `traceroute 10.30.0.10` from `NY-PC` (`10.10.0.10`) to see spoke-to-spoke traffic hair-pinning through `R-HQ-Hub`.',
      'Ping `HQ-ERP` (`10.0.0.50`) from any branch office.',
    ],
    build: () => ({
      format: 'netlab', version: 1, name: 'Hub and Spoke WAN',
      nodes: [
        N(1, 'router', 'R-HQ-Hub', 460, 210, [
          ip('10.250.0.1', 30), ip('10.250.0.5', 30), ip('10.250.0.9', 30), ip('10.0.0.1', 24),
        ], [
          rt('10.10.0.0', 24, '10.250.0.2', 0),
          rt('10.20.0.0', 24, '10.250.0.6', 1),
          rt('10.30.0.0', 24, '10.250.0.10', 2),
        ], { portCount: 4 }),
        N(2, 'server', 'HQ-ERP', 460, 70, [ip('10.0.0.50', 24)], gw('10.0.0.1')),
        N(3, 'router', 'R-NY', 180, 370, [
          ip('10.250.0.2', 30), ip('10.10.0.1', 24),
        ], [
          rt('0.0.0.0', 0, '10.250.0.1', 0),
        ]),
        N(4, 'router', 'R-LDN', 460, 370, [
          ip('10.250.0.6', 30), ip('10.20.0.1', 24),
        ], [
          rt('0.0.0.0', 0, '10.250.0.5', 0),
        ]),
        N(5, 'router', 'R-TYO', 740, 370, [
          ip('10.250.0.10', 30), ip('10.30.0.1', 24),
        ], [
          rt('0.0.0.0', 0, '10.250.0.9', 0),
        ]),
        N(6, 'pc', 'NY-PC', 180, 520, [ip('10.10.0.10', 24)], gw('10.10.0.1')),
        N(7, 'laptop', 'LDN-Lap', 460, 520, [ip('10.20.0.10', 24)], gw('10.20.0.1')),
        N(8, 'server', 'TYO-Srv', 740, 520, [ip('10.30.0.10', 24)], gw('10.30.0.1')),
      ],
      links: [
        L(1, 3, 2, 0),
        L(1, 0, 3, 0),
        L(1, 1, 4, 0),
        L(1, 2, 5, 0),
        L(3, 1, 6, 0),
        L(4, 1, 7, 0),
        L(5, 1, 8, 0),
      ],
      shapes: [
        S(1, 'rect', 340, 20, 240, 240, '#a58bff', 'HQ Hub · 10.0.0.0/24'),
        S(2, 'rect', 80, 320, 200, 260, '#4c9bff', 'New York · 10.10.0.0/24'),
        S(3, 'rect', 360, 320, 200, 260, '#3ccf7e', 'London · 10.20.0.0/24'),
        S(4, 'rect', 640, 320, 200, 260, '#f2b84b', 'Tokyo · 10.30.0.0/24'),
      ],
      notes: [
        { id: 1, x: 40, y: 40, text: 'Hub-and-Spoke WAN\n• Spokes use a single 0.0.0.0/0 route → R-HQ-Hub\n• Run "traceroute 10.30.0.10" on NY-PC' },
      ],
    }),
  },

  {
    id: 'linear-5hop-chain',
    title: '5-Router Linear Chain & Traceroute TTL Lab',
    level: 'intermediate',
    category: 'Routing',
    summary: 'Five routers wired in series across four /30 transit links — built specifically to visualize TTL decrement and ICMP Time Exceeded in Traceroute.',
    tags: ['Traceroute', 'TTL Expiration', '5-Hop Chain', 'ICMP Time Exceeded'],
    objectives: [
      'Double-click `Source-PC` (`10.1.0.10`) and run `traceroute 10.6.0.20` to see every router hop (`R1` → `R2` → `R3` → `R4` → `R5`) reply in order.',
      'Switch to Simulation mode and watch a packet traverse all 5 routers hop by hop.',
      'Try deleting one return route on `R4` (`clear route 10.1.0.0/24`) to diagnose asymmetric routing failures.',
    ],
    build: () => ({
      format: 'netlab', version: 1, name: '5-Hop Linear Traceroute Lab',
      nodes: [
        N(1, 'pc', 'Source-PC', 70, 280, [ip('10.1.0.10', 24)], gw('10.1.0.1')),
        N(2, 'router', 'R1', 200, 280, [ip('10.1.0.1', 24), ip('10.12.0.1', 30)], [
          rt('0.0.0.0', 0, '10.12.0.2', 1),
        ]),
        N(3, 'router', 'R2', 340, 280, [ip('10.12.0.2', 30), ip('10.23.0.1', 30)], [
          rt('10.1.0.0', 24, '10.12.0.1', 0),
          rt('0.0.0.0', 0, '10.23.0.2', 1),
        ]),
        N(4, 'router', 'R3', 480, 280, [ip('10.23.0.2', 30), ip('10.34.0.1', 30)], [
          rt('10.1.0.0', 24, '10.23.0.1', 0),
          rt('10.12.0.0', 30, '10.23.0.1', 0),
          rt('0.0.0.0', 0, '10.34.0.2', 1),
        ]),
        N(5, 'router', 'R4', 620, 280, [ip('10.34.0.2', 30), ip('10.45.0.1', 30)], [
          rt('0.0.0.0', 0, '10.34.0.1', 0),
          rt('10.6.0.0', 24, '10.45.0.2', 1),
        ]),
        N(6, 'router', 'R5', 760, 280, [ip('10.45.0.2', 30), ip('10.6.0.1', 24)], [
          rt('0.0.0.0', 0, '10.45.0.1', 0),
        ]),
        N(7, 'server', 'Target-Srv', 900, 280, [ip('10.6.0.20', 24)], gw('10.6.0.1')),
      ],
      links: [
        L(1, 0, 2, 0),
        L(2, 1, 3, 0),
        L(3, 1, 4, 0),
        L(4, 1, 5, 0),
        L(5, 1, 6, 0),
        L(6, 1, 7, 0),
      ],
      shapes: [
        S(1, 'rect', 20, 190, 940, 180, '#4c9bff', '5-Hop Linear Transit Chain · R1 → R2 → R3 → R4 → R5'),
      ],
      notes: [
        { id: 1, x: 240, y: 70, text: '5-Hop Traceroute Lab\n• Open Source-PC → traceroute 10.6.0.20\n• Switch to Simulation mode to watch TTL decrement at each router' },
      ],
    }),
  },

  {
    id: 'redundant-ring-backbone',
    title: '4-Node Metro Fiber Ring Topology',
    level: 'intermediate',
    category: 'WAN & Routing',
    summary: 'Four metropolitan Point-of-Presence (POP) routers arranged in a resilient ring (North, East, South, West) serving local POP subnets.',
    tags: ['Ring Topology', 'Metro Fiber', 'Path Redundancy', 'Rerouting'],
    objectives: [
      'Ping from `North-Host` (`10.1.0.10`) to `South-Srv` (`10.3.0.20`) and run `traceroute 10.3.0.20` to see whether traffic flows clockwise (`POP-East`) or counter-clockwise (`POP-West`).',
      'Notice that `POP-North → POP-East → POP-South` carries forward traffic while `POP-South → POP-West → POP-North` can carry return traffic (asymmetric ring routing).',
      'Cut the `POP-North ↔ POP-East` fiber link and update `POP-North` to route `10.3.0.0/24` via `POP-West` (`10.0.41.1`).',
    ],
    build: () => ({
      format: 'netlab', version: 1, name: 'Metro Fiber Ring',
      nodes: [
        N(1, 'router', 'POP-North', 460, 120, [
          ip('10.0.12.1', 30), ip('10.0.41.2', 30), ip('10.1.0.1', 24),
        ], [
          rt('10.2.0.0', 24, '10.0.12.2', 0),
          rt('10.3.0.0', 24, '10.0.12.2', 0),
          rt('10.4.0.0', 24, '10.0.41.1', 1),
        ]),
        N(2, 'router', 'POP-East', 700, 300, [
          ip('10.0.12.2', 30), ip('10.0.23.1', 30), ip('10.2.0.1', 24),
        ], [
          rt('10.1.0.0', 24, '10.0.12.1', 0),
          rt('10.3.0.0', 24, '10.0.23.2', 1),
          rt('10.4.0.0', 24, '10.0.23.2', 1),
        ]),
        N(3, 'router', 'POP-South', 460, 480, [
          ip('10.0.23.2', 30), ip('10.0.34.1', 30), ip('10.3.0.1', 24),
        ], [
          rt('10.2.0.0', 24, '10.0.23.1', 0),
          rt('10.4.0.0', 24, '10.0.34.2', 1),
          rt('10.1.0.0', 24, '10.0.23.1', 0),
        ]),
        N(4, 'router', 'POP-West', 220, 300, [
          ip('10.0.34.2', 30), ip('10.0.41.1', 30), ip('10.4.0.1', 24),
        ], [
          rt('10.3.0.0', 24, '10.0.34.1', 0),
          rt('10.1.0.0', 24, '10.0.41.2', 1),
          rt('10.2.0.0', 24, '10.0.41.2', 1),
        ]),
        N(5, 'pc', 'North-Host', 460, 20, [ip('10.1.0.10', 24)], gw('10.1.0.1')),
        N(6, 'pc', 'East-Host', 870, 300, [ip('10.2.0.10', 24)], gw('10.2.0.1')),
        N(7, 'server', 'South-Srv', 460, 590, [ip('10.3.0.20', 24)], gw('10.3.0.1')),
        N(8, 'laptop', 'West-Lap', 50, 300, [ip('10.4.0.10', 24)], gw('10.4.0.1')),
      ],
      links: [
        L(1, 0, 2, 0),
        L(2, 1, 3, 0),
        L(3, 1, 4, 0),
        L(4, 1, 1, 1),
        L(1, 2, 5, 0),
        L(2, 2, 6, 0),
        L(3, 2, 7, 0),
        L(4, 2, 8, 0),
      ],
      shapes: [
        S(1, 'ellipse', 180, 80, 560, 440, '#2fd0c0', 'Metro DWDM Fiber Ring'),
      ],
      notes: [
        { id: 1, x: 40, y: 40, text: 'Metro Ring Lab\n• Clockwise: North → East → South → West\n• Ping 10.3.0.20 from North-Host' },
      ],
    }),
  },

  {
    id: 'segmented-microservices',
    title: '3-Tier Web / App / Database Microservices',
    level: 'intermediate',
    category: 'Data Center',
    summary: 'Production web application architecture separating External Clients, Frontend Reverse Proxies, Dual-NIC App Workers, and an isolated Database tier.',
    tags: ['3-Tier App', 'Microservices', 'Multi-NIC Worker', 'Backend Isolation'],
    objectives: [
      'Ping `Web-LB` (`172.16.10.10`) from `Client-Browser` (`192.168.50.11`) across `R-Ingress`.',
      'Inspect `App-Worker1` and `App-Worker2` — each worker is dual-homed to the Web Tier (`172.16.10.0/24` on `eth0`) and the private Database Tier (`172.16.20.0/24` on `eth1`).',
      'Verify that `Postgres-Primary` (`172.16.20.50`) is completely unreachable from `Client-Browser`, but reachable from `App-Worker1` (`ping 172.16.20.50`).',
    ],
    build: () => ({
      format: 'netlab', version: 1, name: '3-Tier Web App Architecture',
      nodes: [
        N(1, 'pc', 'Client-1', 90, 200, [ip('192.168.50.11', 24)], gw('192.168.50.1')),
        N(2, 'laptop', 'Client-2', 90, 360, [ip('192.168.50.12', 24)], gw('192.168.50.1')),
        N(3, 'switch', 'SW-Clients', 220, 280),
        N(4, 'router', 'R-Ingress', 360, 280, [
          ip('192.168.50.1', 24), ip('172.16.10.1', 24),
        ]),
        N(5, 'switch', 'SW-WebTier', 510, 280),
        N(6, 'server', 'Web-LB', 510, 120, [ip('172.16.10.10', 24)], gw('172.16.10.1')),
        N(7, 'server', 'App-Worker1', 660, 200, [ip('172.16.10.21', 24), ip('172.16.20.21', 24)], gw('172.16.10.1', 0), { portCount: 2 }),
        N(8, 'server', 'App-Worker2', 660, 360, [ip('172.16.10.22', 24), ip('172.16.20.22', 24)], gw('172.16.10.1', 0), { portCount: 2 }),
        N(9, 'switch', 'SW-DBTier', 810, 280),
        N(10, 'server', 'Postgres-DB', 930, 200, [ip('172.16.20.50', 24)]),
        N(11, 'server', 'Redis-Cache', 930, 360, [ip('172.16.20.51', 24)]),
      ],
      links: [
        L(1, 0, 3, 1), L(2, 0, 3, 2),
        L(3, 0, 4, 0),
        L(4, 1, 5, 0),
        L(5, 1, 6, 0),
        L(5, 2, 7, 0),
        L(5, 3, 8, 0),
        L(7, 1, 9, 1),
        L(8, 1, 9, 2),
        L(9, 3, 10, 0),
        L(9, 4, 11, 0),
      ],
      shapes: [
        S(1, 'rect', 30, 130, 250, 300, '#4c9bff', 'Client Network · 192.168.50.0/24'),
        S(2, 'rect', 440, 65, 290, 365, '#3ccf7e', 'Web & App Tier · 172.16.10.0/24'),
        S(3, 'rect', 755, 130, 240, 300, '#a58bff', 'Isolated DB Tier · 172.16.20.0/24'),
      ],
      notes: [
        { id: 1, x: 35, y: 40, text: '3-Tier Microservices\n• App-Worker1 & App-Worker2 bridge Web (eth0) and Isolated DB (eth1)\n• Postgres-DB & Redis-Cache have no external route' },
      ],
    }),
  },

  /* ─────────────────────────────── ADVANCED ─────────────────────────────── */
  {
    id: 'campus-3tier',
    title: 'Campus 3-Tier Hierarchy (Core / Dist / Access)',
    level: 'advanced',
    category: 'Enterprise',
    summary: 'Hierarchical enterprise campus design with Core, Distribution, and Access layers serving Floor 1, Floor 2, and a Campus Data Center.',
    tags: ['3-Tier Campus', 'Core/Dist/Access', 'Enterprise L3', 'Server Farm'],
    objectives: [
      'Trace end-to-end traffic from Floor-1 workstations (`10.10.1.0/24`) to Campus Data Center servers (`10.20.0.0/24`).',
      'Inspect routing tables on `R-Core`, `R-Dist-West`, and `R-Dist-East` to see how summary and subnet routes converge.',
      'Simulate a distribution link outage and observe traffic impact across access closets.',
    ],
    build: () => ({
      format: 'netlab', version: 1, name: 'Campus 3-Tier Network',
      nodes: [
        N(1, 'router', 'R-Core', 460, 80, [
          ip('10.0.1.1', 30), ip('10.0.2.1', 30), ip('10.0.3.1', 30),
        ], [
          rt('10.10.1.0', 24, '10.0.1.2', 0),
          rt('10.10.2.0', 24, '10.0.1.2', 0),
          rt('10.20.0.0', 24, '10.0.2.2', 1),
        ]),
        N(2, 'router', 'R-Dist-BldgA', 260, 210, [
          ip('10.0.1.2', 30), ip('10.10.1.1', 24), ip('10.10.2.1', 24), ip('10.0.12.1', 30),
        ], [
          rt('0.0.0.0', 0, '10.0.1.1', 0),
          rt('10.20.0.0', 24, '10.0.12.2', 3),
        ]),
        N(3, 'router', 'R-Dist-DC', 660, 210, [
          ip('10.0.2.2', 30), ip('10.20.0.1', 24), ip('10.0.12.2', 30),
        ], [
          rt('0.0.0.0', 0, '10.0.2.1', 0),
          rt('10.10.1.0', 24, '10.0.12.1', 2),
          rt('10.10.2.0', 24, '10.0.12.1', 2),
        ]),
        N(4, 'switch', 'SW-Floor1', 150, 360),
        N(5, 'switch', 'SW-Floor2', 370, 360),
        N(6, 'switch', 'SW-DC-ToR', 660, 360),
        N(7, 'pc', 'F1-Desk1', 90, 500, [ip('10.10.1.11', 24)], gw('10.10.1.1')),
        N(8, 'laptop', 'F1-Wifi1', 210, 500, [ip('10.10.1.12', 24)], gw('10.10.1.1')),
        N(9, 'pc', 'F2-Desk1', 320, 500, [ip('10.10.2.11', 24)], gw('10.10.2.1')),
        N(10, 'laptop', 'F2-Wifi1', 430, 500, [ip('10.10.2.12', 24)], gw('10.10.2.1')),
        N(11, 'server', 'DC-Auth', 590, 500, [ip('10.20.0.10', 24)], gw('10.20.0.1')),
        N(12, 'server', 'DC-Intranet', 730, 500, [ip('10.20.0.20', 24)], gw('10.20.0.1')),
      ],
      links: [
        L(1, 0, 2, 0),
        L(1, 1, 3, 0),
        L(2, 3, 3, 2),
        L(2, 1, 4, 0),
        L(2, 2, 5, 0),
        L(3, 1, 6, 0),
        L(4, 1, 7, 0), L(4, 2, 8, 0),
        L(5, 1, 9, 0), L(5, 2, 10, 0),
        L(6, 1, 11, 0), L(6, 2, 12, 0),
      ],
      shapes: [
        S(1, 'rect', 350, 30, 220, 95, '#a58bff', 'Core Layer'),
        S(2, 'rect', 160, 155, 600, 105, '#4c9bff', 'Distribution Layer (L3 Interconnect)'),
        S(3, 'rect', 50, 305, 440, 255, '#3ccf7e', 'Building A Access · Floors 1 & 2'),
        S(4, 'rect', 540, 305, 250, 255, '#f2b84b', 'Campus Data Center · 10.20.0.0/24'),
      ],
      notes: [
        { id: 1, x: 50, y: 40, text: 'Lab 07 · Campus 3-Tier Design\n• Cross-link 10.0.12.0/30 between R-Dist-BldgA & R-Dist-DC\n• Ping DC-Intranet (10.20.0.20) from F1-Desk1 (10.10.1.11)' },
      ],
    }),
  },

  {
    id: 'enterprise-dual-isp',
    title: 'Dual-ISP Enterprise Edge & Internet Peering',
    level: 'advanced',
    category: 'WAN & ISP',
    summary: 'Enterprise edge router homed to two upstream ISPs (ISP-A and ISP-B) converging on an Internet Exchange Point (IXP) and public Anycast services.',
    tags: ['Dual-ISP', 'WAN Edge', 'IXP', 'Redundancy'],
    objectives: [
      'Ping Public-DNS (`8.8.8.8`) and Public-Web (`1.1.1.10`) from `Corp-Workstation` (`192.168.100.11`).',
      'Capture on `R-Edge` uplinks (`g0/0` to ISP-A and `g0/1` to ISP-B) to see which upstream carries traffic.',
      'Simulate an ISP-A fiber cut by unplugging `R-Edge g0/0 ↔ ISP-A g0/0` and switching the default route on `R-Edge` to `203.0.113.5` (ISP-B).',
    ],
    build: () => ({
      format: 'netlab', version: 1, name: 'Dual-ISP Enterprise Edge',
      nodes: [
        N(1, 'router', 'R-Edge', 460, 350, [
          ip('203.0.113.2', 30), ip('203.0.113.6', 30), ip('192.168.100.1', 24), ip('172.20.0.1', 24),
        ], [
          rt('0.0.0.0', 0, '203.0.113.1', 0),
        ]),
        N(2, 'router', 'ISP-A', 280, 210, [
          ip('203.0.113.1', 30), ip('198.51.100.1', 30),
        ], [
          rt('192.168.100.0', 24, '203.0.113.2', 0),
          rt('172.20.0.0', 24, '203.0.113.2', 0),
          rt('0.0.0.0', 0, '198.51.100.2', 1),
        ]),
        N(3, 'router', 'ISP-B', 640, 210, [
          ip('203.0.113.5', 30), ip('198.51.100.5', 30),
        ], [
          rt('192.168.100.0', 24, '203.0.113.6', 0),
          rt('172.20.0.0', 24, '203.0.113.6', 0),
          rt('0.0.0.0', 0, '198.51.100.6', 1),
        ]),
        N(4, 'router', 'R-IXP', 460, 80, [
          ip('198.51.100.2', 30), ip('198.51.100.6', 30), ip('8.8.8.1', 24), ip('1.1.1.1', 24),
        ], [
          rt('192.168.100.0', 24, '198.51.100.1', 0),
          rt('172.20.0.0', 24, '198.51.100.1', 0),
        ]),
        N(5, 'server', 'Public-DNS', 230, 80, [ip('8.8.8.8', 24)], gw('8.8.8.1')),
        N(6, 'server', 'Public-Web', 690, 80, [ip('1.1.1.10', 24)], gw('1.1.1.1')),
        N(7, 'switch', 'SW-Corp', 290, 480),
        N(8, 'switch', 'SW-DMZ', 630, 480),
        N(9, 'pc', 'Corp-WS1', 190, 580, [ip('192.168.100.11', 24)], gw('192.168.100.1')),
        N(10, 'laptop', 'Corp-Lap1', 350, 580, [ip('192.168.100.12', 24)], gw('192.168.100.1')),
        N(11, 'server', 'Edge-Mail', 570, 580, [ip('172.20.0.25', 24)], gw('172.20.0.1')),
        N(12, 'server', 'Edge-Web', 710, 580, [ip('172.20.0.80', 24)], gw('172.20.0.1')),
      ],
      links: [
        L(1, 0, 2, 0),
        L(1, 1, 3, 0),
        L(2, 1, 4, 0),
        L(3, 1, 4, 1),
        L(4, 2, 5, 0),
        L(4, 3, 6, 0),
        L(1, 2, 7, 0),
        L(1, 3, 8, 0),
        L(7, 1, 9, 0), L(7, 2, 10, 0),
        L(8, 1, 11, 0), L(8, 2, 12, 0),
      ],
      shapes: [
        S(1, 'rect', 160, 25, 600, 240, '#a58bff', 'Upstream Internet & Transit ISPs'),
        S(2, 'rect', 130, 300, 660, 340, '#4c9bff', 'Enterprise Premises (Corp LAN + Public DMZ)'),
      ],
      notes: [
        { id: 1, x: 30, y: 310, text: 'Lab 08 · Dual-ISP Failover\n• Primary path: R-Edge → ISP-A → R-IXP\n• Backup path: R-Edge → ISP-B → R-IXP\n• Ping 8.8.8.8 or 1.1.1.10 from Corp-WS1' },
      ],
    }),
  },

  {
    id: 'isp-peering-ixp',
    title: 'Internet Exchange Point (IXP) & 4-AS Peering Fabric',
    level: 'advanced',
    category: 'WAN & ISP',
    summary: 'Shared Layer-2 Internet Exchange switch (198.32.160.0/24) interconnecting four Autonomous Systems (ISP-US, ISP-EU, Cloud-CDN, and Enterprise-AS).',
    tags: ['IXP Peering', 'BGP Concept', 'Shared L2 Exchange', 'Autonomous Systems'],
    objectives: [
      'Explore how `SW-IXP-Fabric` allows `AS101-US`, `AS202-EU`, `AS303-CDN`, and `AS404-Ent` to peer directly in `198.32.160.0/24` with a single hop across the exchange.',
      'Ping `CDN-Anycast` (`103.21.244.10`) from `US-Subscriber` (`45.10.0.11`) and `EU-Subscriber` (`81.20.0.11`).',
      'Capture on `SW-IXP-Fabric` to observe direct bilateral peering frames between Autonomous Systems.',
    ],
    build: () => ({
      format: 'netlab', version: 1, name: 'IXP Bilateral Peering Fabric',
      nodes: [
        N(1, 'switch', 'SW-IXP-Fabric', 460, 280),
        N(2, 'router', 'AS101-US', 220, 140, [
          ip('198.32.160.1', 24), ip('45.10.0.1', 24),
        ], [
          rt('81.20.0.0', 24, '198.32.160.2', 0),
          rt('103.21.244.0', 24, '198.32.160.3', 0),
          rt('199.40.0.0', 24, '198.32.160.4', 0),
        ]),
        N(3, 'router', 'AS202-EU', 700, 140, [
          ip('198.32.160.2', 24), ip('81.20.0.1', 24),
        ], [
          rt('45.10.0.0', 24, '198.32.160.1', 0),
          rt('103.21.244.0', 24, '198.32.160.3', 0),
          rt('199.40.0.0', 24, '198.32.160.4', 0),
        ]),
        N(4, 'router', 'AS303-CDN', 220, 420, [
          ip('198.32.160.3', 24), ip('103.21.244.1', 24),
        ], [
          rt('45.10.0.0', 24, '198.32.160.1', 0),
          rt('81.20.0.0', 24, '198.32.160.2', 0),
          rt('199.40.0.0', 24, '198.32.160.4', 0),
        ]),
        N(5, 'router', 'AS404-Ent', 700, 420, [
          ip('198.32.160.4', 24), ip('199.40.0.1', 24),
        ], [
          rt('45.10.0.0', 24, '198.32.160.1', 0),
          rt('81.20.0.0', 24, '198.32.160.2', 0),
          rt('103.21.244.0', 24, '198.32.160.3', 0),
        ]),
        N(6, 'pc', 'US-Subscriber', 70, 140, [ip('45.10.0.11', 24)], gw('45.10.0.1')),
        N(7, 'laptop', 'EU-Subscriber', 860, 140, [ip('81.20.0.11', 24)], gw('81.20.0.1')),
        N(8, 'server', 'CDN-EdgeNode', 70, 420, [ip('103.21.244.10', 24)], gw('103.21.244.1')),
        N(9, 'server', 'Ent-Portal', 860, 420, [ip('199.40.0.20', 24)], gw('199.40.0.1')),
      ],
      links: [
        L(2, 0, 1, 0),
        L(3, 0, 1, 1),
        L(4, 0, 1, 2),
        L(5, 0, 1, 3),
        L(2, 1, 6, 0),
        L(3, 1, 7, 0),
        L(4, 1, 8, 0),
        L(5, 1, 9, 0),
      ],
      shapes: [
        S(1, 'rect', 340, 210, 240, 140, '#a58bff', 'IXP Peering LAN · 198.32.160.0/24'),
        S(2, 'rect', 20, 75, 270, 130, '#4c9bff', 'AS101 · 45.10.0.0/24'),
        S(3, 'rect', 630, 75, 290, 130, '#3ccf7e', 'AS202 · 81.20.0.0/24'),
        S(4, 'rect', 20, 355, 270, 130, '#f2b84b', 'AS303 CDN · 103.21.244.0/24'),
        S(5, 'rect', 630, 355, 290, 130, '#2fd0c0', 'AS404 Enterprise · 199.40.0.0/24'),
      ],
      notes: [
        { id: 1, x: 320, y: 60, text: 'Internet Exchange Point (IXP)\n• All 4 border routers peer on 198.32.160.0/24 via SW-IXP-Fabric\n• Any AS reaches any other AS in 2 router hops' },
      ],
    }),
  },

  {
    id: 'industrial-ot-purdue',
    title: 'Industrial ICS / SCADA Purdue Model (Levels 1–4)',
    level: 'advanced',
    category: 'Security & OT',
    summary: 'Defense-in-depth Industrial Control System (ICS) topology implementing Purdue Levels 4 (Enterprise IT), 3.5 (IDMZ Jump Host), 3 (Site Ops / Historian), and 1–2 (SCADA / PLC Control).',
    tags: ['Purdue Model', 'OT / SCADA', 'Industrial DMZ', 'Multi-NIC Historian'],
    objectives: [
      'Examine how `IDMZ-Jump` (`eth0` in Enterprise IT, `eth1` in Industrial DMZ) and `OT-Historian` (`eth0` in IDMZ, `eth1` in Plant Ops) enforce strict conduit boundaries.',
      'Verify that `Corp-Analyst` (`10.40.0.11`) can reach `IDMZ-Jump` (`10.35.0.10`), while `SCADA-HMI` (`10.20.0.10`) and `PLC-Controller` (`10.20.0.50`) remain isolated on the plant floor.',
      'From `OT-Historian`, ping both `IDMZ-Jump` (`10.35.0.10`) and `PLC-Controller` (`10.20.0.50`).',
    ],
    build: () => ({
      format: 'netlab', version: 1, name: 'Industrial ICS Purdue Model',
      nodes: [
        N(1, 'pc', 'Corp-Analyst', 150, 90, [ip('10.40.0.11', 24)], gw('10.40.0.1')),
        N(2, 'switch', 'SW-L4-Corp', 340, 90),
        N(3, 'router', 'FW-IT-IDMZ', 540, 90, [
          ip('10.40.0.1', 24), ip('10.35.0.1', 24),
        ]),
        N(4, 'switch', 'SW-L35-IDMZ', 540, 230),
        N(5, 'server', 'IDMZ-Jump', 310, 230, [ip('10.35.0.10', 24), ip('10.40.0.50', 24)], gw('10.35.0.1', 0), { portCount: 2 }),
        N(6, 'server', 'OT-Historian', 540, 370, [ip('10.35.0.20', 24), ip('10.30.0.20', 24)], gw('10.30.0.1', 1), { portCount: 2 }),
        N(7, 'switch', 'SW-L3-Ops', 540, 490),
        N(8, 'router', 'FW-Cell-Zone', 320, 490, [
          ip('10.30.0.1', 24), ip('10.20.0.1', 24),
        ], [
          rt('10.35.0.0', 24, '10.30.0.20', 0),
        ]),
        N(9, 'switch', 'SW-L2-Cell', 140, 490),
        N(10, 'pc', 'SCADA-HMI', 80, 600, [ip('10.20.0.10', 24)], gw('10.20.0.1')),
        N(11, 'server', 'PLC-Rack1', 220, 600, [ip('10.20.0.50', 24)], gw('10.20.0.1')),
      ],
      links: [
        L(1, 0, 2, 1),
        L(2, 0, 3, 0),
        L(2, 2, 5, 1),
        L(3, 1, 4, 0),
        L(4, 1, 5, 0),
        L(4, 2, 6, 0),
        L(6, 1, 7, 0),
        L(7, 1, 8, 0),
        L(8, 1, 9, 0),
        L(9, 1, 10, 0),
        L(9, 2, 11, 0),
      ],
      shapes: [
        S(1, 'rect', 70, 35, 560, 110, '#4c9bff', 'Level 4 · Enterprise IT (10.40.0.0/24)'),
        S(2, 'rect', 220, 175, 410, 110, '#f2b84b', 'Level 3.5 · Industrial DMZ (10.35.0.0/24)'),
        S(3, 'rect', 240, 320, 390, 225, '#a58bff', 'Level 3 · Site Operations & Historian (10.30.0.0/24)'),
        S(4, 'rect', 30, 440, 250, 215, '#e5534b', 'Levels 1–2 · SCADA & PLC Control Cell (10.20.0.0/24)'),
      ],
      notes: [
        { id: 1, x: 660, y: 180, text: 'Purdue ICS Reference Model\n• L4 Enterprise IT never routes directly to L1/L2 PLCs\n• OT-Historian bridges L3.5 IDMZ (eth0) and L3 Ops (eth1)' },
      ],
    }),
  },

  {
    id: 'hospital-clinical-network',
    title: 'Smart Hospital & Medical Imaging (PACS / HL7) Campus',
    level: 'advanced',
    category: 'Enterprise',
    summary: 'Healthcare network segregating Clinical EHR workstations, high-bandwidth Radiology PACS imaging, Guest Wi-Fi, and a multi-NIC HL7 Integration Engine.',
    tags: ['Healthcare IT', 'PACS Imaging', 'HL7 Gateway', 'Network Segmentation'],
    objectives: [
      'Ping `EHR-Server` (`10.80.10.50`) from `Nurse-Station1` (`10.80.1.11`) through `R-Hosp-Core`.',
      'Inspect `HL7-Engine` — it has 3 interfaces connecting `Clinical-EHR` (`10.80.10.100`), `Radiology-PACS` (`10.80.20.100`), and `Diagnostic-Modality` (`10.80.30.100`).',
      'Verify how radiology modalities (`MRI-Scanner`, `CT-Scanner`) transfer images directly to `PACS-Archive` (`10.80.20.50`).',
    ],
    build: () => ({
      format: 'netlab', version: 1, name: 'Smart Hospital Campus',
      nodes: [
        N(1, 'router', 'R-Hosp-Core', 450, 120, [
          ip('10.80.1.1', 24), ip('10.80.10.1', 24), ip('10.80.20.1', 24),
        ]),
        N(2, 'switch', 'SW-WardA', 180, 260),
        N(3, 'switch', 'SW-EHR-DC', 450, 260),
        N(4, 'switch', 'SW-Radiology', 730, 260),
        N(5, 'pc', 'Nurse-Stn1', 100, 420, [ip('10.80.1.11', 24)], gw('10.80.1.1')),
        N(6, 'laptop', 'MD-Tablet', 230, 420, [ip('10.80.1.12', 24)], gw('10.80.1.1')),
        N(7, 'server', 'EHR-Database', 380, 420, [ip('10.80.10.50', 24)], gw('10.80.10.1')),
        N(8, 'server', 'HL7-Engine', 540, 420, [
          ip('10.80.10.100', 24), ip('10.80.20.100', 24),
        ], gw('10.80.10.1', 0), { portCount: 2 }),
        N(9, 'server', 'PACS-Archive', 680, 420, [ip('10.80.20.50', 24)], gw('10.80.20.1')),
        N(10, 'pc', 'MRI-Scanner', 820, 420, [ip('10.80.20.21', 24)], gw('10.80.20.1')),
      ],
      links: [
        L(1, 0, 2, 0),
        L(1, 1, 3, 0),
        L(1, 2, 4, 0),
        L(2, 1, 5, 0), L(2, 2, 6, 0),
        L(3, 1, 7, 0), L(3, 2, 8, 0),
        L(4, 1, 8, 1), L(4, 2, 9, 0), L(4, 3, 10, 0),
      ],
      shapes: [
        S(1, 'rect', 50, 210, 240, 270, '#4c9bff', 'Patient Ward · 10.80.1.0/24'),
        S(2, 'rect', 330, 210, 270, 270, '#3ccf7e', 'Clinical EHR · 10.80.10.0/24'),
        S(3, 'rect', 630, 210, 260, 270, '#a58bff', 'Radiology PACS · 10.80.20.0/24'),
      ],
      notes: [
        { id: 1, x: 50, y: 50, text: 'Hospital Clinical & PACS Network\n• HL7-Engine bridges Clinical EHR (.10.100) and Radiology PACS (.20.100)\n• Ping PACS-Archive (10.80.20.50) from MD-Tablet (10.80.1.12)' },
      ],
    }),
  },

  {
    id: 'content-delivery-anycast',
    title: 'Multi-Region CDN Edge PoPs & Origin Shield',
    level: 'advanced',
    category: 'WAN & ISP',
    summary: 'Global Content Delivery Network with US and EU Edge PoPs serving regional eyeballs and backhauling cache misses through an Origin Shield.',
    tags: ['CDN', 'Edge PoP', 'Origin Shield', 'Multi-Region'],
    objectives: [
      'Ping `US-Edge-Cache` (`10.200.1.10`) from `US-User` (`172.16.1.10`) — notice it resolves in 1 router hop at the local US PoP.',
      'Run `traceroute 10.200.99.50` from `US-User` or `EU-User` to trace a cache-miss backhaul path through `R-Origin-Shield` to `Origin-Storage`.',
      'Inspect `R-Origin-Shield` routing table connecting both trans-oceanic backhaul trunks (`10.254.1.0/30` and `10.254.2.0/30`).',
    ],
    build: () => ({
      format: 'netlab', version: 1, name: 'Multi-Region CDN & Origin Shield',
      nodes: [
        N(1, 'router', 'R-Origin-Shield', 460, 100, [
          ip('10.254.1.1', 30), ip('10.254.2.1', 30), ip('10.200.99.1', 24),
        ], [
          rt('172.16.1.0', 24, '10.254.1.2', 0),
          rt('10.200.1.0', 24, '10.254.1.2', 0),
          rt('172.16.2.0', 24, '10.254.2.2', 1),
          rt('10.200.2.0', 24, '10.254.2.2', 1),
        ]),
        N(2, 'server', 'Origin-Storage', 460, 240, [ip('10.200.99.50', 24)], gw('10.200.99.1')),
        N(3, 'router', 'PoP-US-East', 200, 260, [
          ip('10.254.1.2', 30), ip('10.200.1.1', 24), ip('172.16.1.1', 24),
        ], [
          rt('0.0.0.0', 0, '10.254.1.1', 0),
        ]),
        N(4, 'router', 'PoP-EU-West', 720, 260, [
          ip('10.254.2.2', 30), ip('10.200.2.1', 24), ip('172.16.2.1', 24),
        ], [
          rt('0.0.0.0', 0, '10.254.2.1', 0),
        ]),
        N(5, 'server', 'US-Edge-Cache', 90, 420, [ip('10.200.1.10', 24)], gw('10.200.1.1')),
        N(6, 'pc', 'US-Eyeball', 260, 420, [ip('172.16.1.10', 24)], gw('172.16.1.1')),
        N(7, 'server', 'EU-Edge-Cache', 660, 420, [ip('10.200.2.10', 24)], gw('10.200.2.1')),
        N(8, 'laptop', 'EU-Eyeball', 830, 420, [ip('172.16.2.10', 24)], gw('172.16.2.1')),
      ],
      links: [
        L(1, 2, 2, 0),
        L(1, 0, 3, 0),
        L(1, 1, 4, 0),
        L(3, 1, 5, 0), L(3, 2, 6, 0),
        L(4, 1, 7, 0), L(4, 2, 8, 0),
      ],
      shapes: [
        S(1, 'rect', 350, 40, 220, 260, '#a58bff', 'Origin Shield Tier · 10.200.99.0/24'),
        S(2, 'rect', 30, 210, 300, 280, '#4c9bff', 'US-East Edge PoP'),
        S(3, 'rect', 590, 210, 300, 280, '#3ccf7e', 'EU-West Edge PoP'),
      ],
      notes: [
        { id: 1, x: 40, y: 50, text: 'Multi-Region CDN\n• Local hit: US-Eyeball → US-Edge-Cache (10.200.1.10)\n• Origin miss: US-Edge-Cache → R-Origin-Shield → Origin-Storage (10.200.99.50)' },
      ],
    }),
  },

  /* ─────────────────────────────── EXPERT ─────────────────────────────── */
  {
    id: 'clos-datacenter',
    title: 'Leaf-Spine (Clos) Data Center Fabric',
    level: 'expert',
    category: 'Data Center',
    summary: 'Modern Layer-3 Leaf-Spine Clos fabric with 2 Spine routers, 3 Top-of-Rack Leaf routers, and dual-homed rack servers.',
    tags: ['Leaf-Spine', 'Clos Fabric', 'Data Center', 'East-West Traffic', 'Multi-NIC'],
    objectives: [
      'Explore a full bipartite mesh where every Leaf router connects to every Spine router over routed point-to-point `/30` links.',
      'Ping East-West between `Rack1-Srv1` (`10.100.1.11`) and `Rack3-Srv1` (`10.100.3.11`) across the Spine layer.',
      'Test high availability: Rack2-DualSrv is dual-homed to Rack-2 and Rack-3 (`10.100.2.50` on `eth0` and `10.100.3.50` on `eth1`).',
    ],
    build: () => ({
      format: 'netlab', version: 1, name: 'Leaf-Spine Clos Fabric',
      nodes: [
        N(1, 'router', 'Spine-1', 330, 100, [
          ip('10.255.11.1', 30), ip('10.255.12.1', 30), ip('10.255.13.1', 30),
        ], [
          rt('10.100.1.0', 24, '10.255.11.2', 0),
          rt('10.100.2.0', 24, '10.255.12.2', 1),
          rt('10.100.3.0', 24, '10.255.13.2', 2),
        ], { portCount: 6 }),
        N(2, 'router', 'Spine-2', 630, 100, [
          ip('10.255.21.1', 30), ip('10.255.22.1', 30), ip('10.255.23.1', 30),
        ], [
          rt('10.100.1.0', 24, '10.255.21.2', 0),
          rt('10.100.2.0', 24, '10.255.22.2', 1),
          rt('10.100.3.0', 24, '10.255.23.2', 2),
        ], { portCount: 6 }),
        N(3, 'router', 'Leaf-1', 200, 270, [
          ip('10.255.11.2', 30), ip('10.255.21.2', 30), ip('10.100.1.1', 24),
        ], [
          rt('10.100.2.0', 24, '10.255.11.1', 0),
          rt('10.100.3.0', 24, '10.255.21.1', 1),
        ], { portCount: 4 }),
        N(4, 'router', 'Leaf-2', 480, 270, [
          ip('10.255.12.2', 30), ip('10.255.22.2', 30), ip('10.100.2.1', 24),
        ], [
          rt('10.100.1.0', 24, '10.255.12.1', 0),
          rt('10.100.3.0', 24, '10.255.22.1', 1),
        ], { portCount: 4 }),
        N(5, 'router', 'Leaf-3', 760, 270, [
          ip('10.255.13.2', 30), ip('10.255.23.2', 30), ip('10.100.3.1', 24),
        ], [
          rt('10.100.1.0', 24, '10.255.13.1', 0),
          rt('10.100.2.0', 24, '10.255.23.1', 1),
        ], { portCount: 4 }),
        N(6, 'switch', 'ToR-Rack1', 200, 410),
        N(7, 'switch', 'ToR-Rack2', 480, 410),
        N(8, 'switch', 'ToR-Rack3', 760, 410),
        N(9, 'server', 'Rack1-Srv1', 140, 540, [ip('10.100.1.11', 24)], gw('10.100.1.1')),
        N(10, 'server', 'Rack1-Srv2', 250, 540, [ip('10.100.1.12', 24)], gw('10.100.1.1')),
        N(11, 'server', 'Rack2-Srv1', 420, 540, [ip('10.100.2.11', 24)], gw('10.100.2.1')),
        N(12, 'server', 'Rack2-DualSrv', 580, 540, [ip('10.100.2.50', 24), ip('10.100.3.50', 24)], gw('10.100.2.1', 0), { portCount: 2 }),
        N(13, 'server', 'Rack3-Srv1', 780, 540, [ip('10.100.3.11', 24)], gw('10.100.3.1')),
      ],
      links: [
        L(3, 0, 1, 0), L(3, 1, 2, 0),
        L(4, 0, 1, 1), L(4, 1, 2, 1),
        L(5, 0, 1, 2), L(5, 1, 2, 2),
        L(3, 2, 6, 0),
        L(4, 2, 7, 0),
        L(5, 2, 8, 0),
        L(6, 1, 9, 0), L(6, 2, 10, 0),
        L(7, 1, 11, 0), L(7, 2, 12, 0),
        L(8, 1, 12, 1), L(8, 2, 13, 0),
      ],
      shapes: [
        S(1, 'rect', 220, 40, 520, 110, '#a58bff', 'Spine Layer · L3 Clos Core'),
        S(2, 'rect', 120, 215, 720, 110, '#4c9bff', 'Leaf Layer · L3 Top-of-Rack Gateways'),
        S(3, 'rect', 90, 365, 210, 235, '#3ccf7e', 'Rack 1 · 10.100.1.0/24'),
        S(4, 'rect', 360, 365, 270, 235, '#f2b84b', 'Rack 2 · 10.100.2.0/24'),
        S(5, 'rect', 670, 365, 190, 235, '#2fd0c0', 'Rack 3 · 10.100.3.0/24'),
      ],
      notes: [
        { id: 1, x: 20, y: 40, text: 'Lab 09 · Leaf-Spine Data Center\n• Leaf-1..3 each uplift to Spine-1 & Spine-2\n• Rack2-DualSrv is dual-homed across Rack 2 & Rack 3\n• Ping 10.100.3.11 from Rack1-Srv1' },
      ],
    }),
  },

  {
    id: 'hybrid-emulation-lab',
    title: 'Multi-Vendor Emulation: Cisco IOS + MikroTik + x86 VM',
    level: 'expert',
    category: 'Emulated Appliances',
    summary: 'Full multi-vendor integration lab combining Cisco 7200 IOS (Dynamips), MikroTik RouterOS (v86), a multi-NIC Linux x86 VM, and simulated LAN segments.',
    tags: ['Cisco IOS', 'MikroTik RouterOS', 'x86 VM', 'Multi-Vendor', 'Multi-NIC'],
    objectives: [
      'Attach your Cisco 7200 `.bin` image to `IOS-Core` and your RouterOS v6 `.iso`/disk to `MT-Edge` via the Images manager.',
      'Configure `IOS-Core f0/0` (`10.0.0.1/30`) and `R-Sim g0/0` (`10.0.0.2/30`) to exchange packets between real Cisco IOS and the Rust/WASM fabric.',
      'Boot `MT-Edge` (4 NICs) and `VM-Bastion` (3 NICs) to bridge simulated hosts with real x86 network operating systems.',
    ],
    build: () => ({
      format: 'netlab', version: 1, name: 'Multi-Vendor Emulation Lab',
      nodes: [
        N(1, 'ios', 'IOS-Core', 310, 120, [], [], { portCount: 4 }),
        N(2, 'routeros', 'MT-Edge', 610, 120, [], [], { portCount: 4 }),
        N(3, 'router', 'R-Sim', 310, 280, [
          ip('10.0.0.2', 30), ip('10.100.10.1', 24), ip('10.100.20.1', 24),
        ], [
          rt('0.0.0.0', 0, '10.0.0.1', 0),
        ]),
        N(4, 'x86', 'VM-Bastion', 610, 280, [], [], { portCount: 3 }),
        N(5, 'switch', 'SW-LAN1', 230, 430),
        N(6, 'switch', 'SW-LAN2', 520, 430),
        N(7, 'pc', 'PC-Lab1', 140, 550, [ip('10.100.10.11', 24)], gw('10.100.10.1')),
        N(8, 'server', 'Srv-Lab1', 280, 550, [ip('10.100.10.20', 24)], gw('10.100.10.1')),
        N(9, 'pc', 'PC-Lab2', 460, 550, [ip('10.100.20.11', 24)], gw('10.100.20.1')),
        N(10, 'laptop', 'Lap-Lab2', 600, 550, [ip('10.100.20.12', 24)], gw('10.100.20.1')),
      ],
      links: [
        L(1, 0, 3, 0),
        L(1, 1, 2, 0),
        L(2, 1, 4, 0),
        L(3, 1, 5, 0),
        L(3, 2, 6, 0),
        L(4, 1, 5, 3),
        L(4, 2, 6, 3),
        L(5, 1, 7, 0), L(5, 2, 8, 0),
        L(6, 1, 9, 0), L(6, 2, 10, 0),
      ],
      shapes: [
        S(1, 'rect', 210, 50, 510, 145, '#2fd0c0', 'Real Emulated Control Plane · Cisco 7200 + MikroTik RouterOS'),
        S(2, 'rect', 90, 380, 600, 230, '#4c9bff', 'Simulated Switching Fabric · 10.100.10.0/24 & 10.100.20.0/24'),
      ],
      notes: [
        { id: 1, x: 30, y: 60, text: 'Lab 10 · Multi-Vendor Emulation\n• IOS-Core: f0/0 → R-Sim (10.0.0.0/30), f1/0 → MT-Edge ether1\n• MT-Edge: ether2 → VM-Bastion eth0\n• VM-Bastion bridges eth1 (SW-LAN1) & eth2 (SW-LAN2)' },
      ],
    }),
  },

  {
    id: 'full-mesh-core',
    title: '4-Region Full-Mesh Global Core Backbone',
    level: 'expert',
    category: 'WAN & ISP',
    summary: 'Complete K4 full-mesh core (6 inter-region /30 trunks connecting NYC, Frankfurt, Singapore, and Tokyo) offering direct 1-hop paths between every pair of global regions.',
    tags: ['Full Mesh', 'Global Backbone', 'K4 Graph', 'Zero SPOF'],
    objectives: [
      'Verify that every core router (`Core-NYC`, `Core-FRA`, `Core-SIN`, `Core-TYO`) has a direct `/30` link to all 3 other regions (`4 × 3 / 2 = 6` core trunks).',
      'Ping from `NYC-Trading` (`10.1.0.10`) to `SIN-Matching` (`10.3.0.10`) over the direct `Core-NYC ↔ Core-SIN` diagonal low-latency trunk (`172.31.13.0/30`).',
      'Disable any direct trunk and reroute through an intermediate region without losing global connectivity.',
    ],
    build: () => ({
      format: 'netlab', version: 1, name: '4-Region Full-Mesh Core',
      nodes: [
        N(1, 'router', 'Core-NYC', 290, 150, [
          ip('172.31.12.1', 30), ip('172.31.13.1', 30), ip('172.31.14.1', 30), ip('10.1.0.1', 24),
        ], [
          rt('10.2.0.0', 24, '172.31.12.2', 0),
          rt('10.3.0.0', 24, '172.31.13.2', 1),
          rt('10.4.0.0', 24, '172.31.14.2', 2),
        ], { portCount: 4 }),
        N(2, 'router', 'Core-FRA', 650, 150, [
          ip('172.31.12.2', 30), ip('172.31.23.1', 30), ip('172.31.24.1', 30), ip('10.2.0.1', 24),
        ], [
          rt('10.1.0.0', 24, '172.31.12.1', 0),
          rt('10.3.0.0', 24, '172.31.23.2', 1),
          rt('10.4.0.0', 24, '172.31.24.2', 2),
        ], { portCount: 4 }),
        N(3, 'router', 'Core-SIN', 650, 430, [
          ip('172.31.13.2', 30), ip('172.31.23.2', 30), ip('172.31.34.1', 30), ip('10.3.0.1', 24),
        ], [
          rt('10.1.0.0', 24, '172.31.13.1', 0),
          rt('10.2.0.0', 24, '172.31.23.1', 1),
          rt('10.4.0.0', 24, '172.31.34.2', 2),
        ], { portCount: 4 }),
        N(4, 'router', 'Core-TYO', 290, 430, [
          ip('172.31.14.2', 30), ip('172.31.24.2', 30), ip('172.31.34.2', 30), ip('10.4.0.1', 24),
        ], [
          rt('10.1.0.0', 24, '172.31.14.1', 0),
          rt('10.2.0.0', 24, '172.31.24.1', 1),
          rt('10.3.0.0', 24, '172.31.34.1', 2),
        ], { portCount: 4 }),
        N(5, 'server', 'NYC-Trading', 110, 150, [ip('10.1.0.10', 24)], gw('10.1.0.1')),
        N(6, 'server', 'FRA-Exchange', 830, 150, [ip('10.2.0.10', 24)], gw('10.2.0.1')),
        N(7, 'server', 'SIN-Matching', 830, 430, [ip('10.3.0.10', 24)], gw('10.3.0.1')),
        N(8, 'server', 'TYO-Ledger', 110, 430, [ip('10.4.0.10', 24)], gw('10.4.0.1')),
      ],
      links: [
        L(1, 0, 2, 0),
        L(1, 1, 3, 0),
        L(1, 2, 4, 0),
        L(2, 1, 3, 1),
        L(2, 2, 4, 1),
        L(3, 2, 4, 2),
        L(1, 3, 5, 0),
        L(2, 3, 6, 0),
        L(3, 3, 7, 0),
        L(4, 3, 8, 0),
      ],
      shapes: [
        S(1, 'rect', 220, 85, 500, 415, '#a58bff', 'Full-Mesh Core · 6 Sub-Sea /30 Trunks'),
      ],
      notes: [
        { id: 1, x: 40, y: 30, text: 'Global Full-Mesh Backbone\n• 6 core /30 links: every POP has a direct 1-hop trunk to every other POP\n• Ping 10.3.0.10 (Singapore) from NYC-Trading (10.1.0.10)' },
      ],
    }),
  },

  {
    id: 'zero-trust-soc',
    title: 'SOC Cyber Range: HoneyNet & 4-NIC SIEM Sensor',
    level: 'expert',
    category: 'Security & Multi-NIC',
    summary: 'Security Operations Center cyber range with an External Red-Team node, Perimeter Firewall, Deception HoneyNet, Production Vault, and a 4-NIC SIEM Collector.',
    tags: ['SOC Cyber Range', 'HoneyNet', '4-NIC SIEM', 'Zero Trust'],
    objectives: [
      'Inspect `SIEM-Collector` — it has 4 NICs (`eth0` SOC Mgmt, `eth1` HoneyNet Tap, `eth2` Prod LAN Tap, `eth3` Quarantine) so analysts can reach every enclave directly.',
      'From `RedTeam-Kali` (`203.0.113.99`), ping the exposed `Honey-SSH` (`172.30.0.10`) and `Honey-Web` (`172.30.0.20`) decoys while capturing packets.',
      'From `SOC-Analyst` (`192.168.250.10`), communicate with `SIEM-Collector` (`192.168.250.50`) and pivot into the HoneyNet or Production enclave.',
    ],
    build: () => ({
      format: 'netlab', version: 1, name: 'SOC Cyber Range & HoneyNet',
      nodes: [
        N(1, 'laptop', 'RedTeam-Kali', 90, 140, [ip('203.0.113.99', 24)], gw('203.0.113.1')),
        N(2, 'router', 'FW-Perimeter', 310, 140, [
          ip('203.0.113.1', 24), ip('172.30.0.1', 24), ip('10.10.50.1', 24),
        ]),
        N(3, 'switch', 'SW-HoneyNet', 310, 320),
        N(4, 'switch', 'SW-Prod', 600, 140),
        N(5, 'switch', 'SW-SOC-Mgmt', 600, 480),
        N(6, 'server', 'Honey-SSH', 150, 440, [ip('172.30.0.10', 24)], gw('172.30.0.1')),
        N(7, 'server', 'Honey-Web', 310, 480, [ip('172.30.0.20', 24)], gw('172.30.0.1')),
        N(8, 'server', 'Prod-Vault', 810, 100, [ip('10.10.50.10', 24)], gw('10.10.50.1')),
        N(9, 'pc', 'Prod-Workstn', 810, 210, [ip('10.10.50.20', 24)], gw('10.10.50.1')),
        N(10, 'server', 'SIEM-Collector', 600, 320, [
          ip('192.168.250.50', 24), ip('172.30.0.250', 24), ip('10.10.50.250', 24), ip('10.99.99.1', 24),
        ], gw('192.168.250.1', 0), { portCount: 4 }),
        N(11, 'pc', 'SOC-Analyst', 820, 480, [ip('192.168.250.10', 24)], gw('192.168.250.50')),
        N(12, 'x86', 'Sandbox-VM', 820, 340, [], [], { portCount: 2 }),
      ],
      links: [
        L(1, 0, 2, 0),
        L(2, 1, 3, 0),
        L(2, 2, 4, 0),
        L(3, 1, 6, 0),
        L(3, 2, 7, 0),
        L(3, 3, 10, 1),
        L(4, 1, 8, 0),
        L(4, 2, 9, 0),
        L(4, 3, 10, 2),
        L(10, 0, 5, 0),
        L(5, 1, 11, 0),
        L(10, 3, 12, 0),
        L(3, 4, 12, 1),
      ],
      shapes: [
        S(1, 'rect', 30, 75, 150, 130, '#e5534b', 'Untrusted WAN'),
        S(2, 'rect', 90, 270, 300, 275, '#f2b84b', 'Deception HoneyNet · 172.30.0.0/24'),
        S(3, 'rect', 530, 50, 360, 210, '#3ccf7e', 'Production Enclave · 10.10.50.0/24'),
        S(4, 'rect', 500, 270, 400, 275, '#a58bff', 'SOC & Malware Sandbox · 4-NIC SIEM'),
      ],
      notes: [
        { id: 1, x: 40, y: 215, text: 'SOC Cyber Range\n• SIEM-Collector has 4 NICs: SOC Mgmt (eth0), HoneyNet (eth1), Prod (eth2), Sandbox (eth3)\n• RedTeam-Kali can probe Honey-SSH (172.30.0.10)' },
      ],
    }),
  },

  {
    id: 'carrier-mpls-backbone',
    title: 'Service Provider P / PE Core & Multi-Tenant Sites',
    level: 'expert',
    category: 'WAN & ISP',
    summary: 'Service provider transit architecture with 2 Provider Edge (PE) routers, 2 Provider Core (P) routers, and dual multi-site enterprise customers.',
    tags: ['Service Provider', 'P / PE Routers', 'Carrier Core', 'Multi-Tenant'],
    objectives: [
      'Run `traceroute 10.100.2.10` from `CustA-HQ` (`10.100.1.10`) to trace the 4-router path: `PE-West → P-Core1 → PE-East → CustA-DR`.',
      'Run `traceroute 10.200.2.10` from `CustB-West` (`10.200.1.10`) and notice how `PE-West` steers Customer B traffic across `P-Core2` (`172.16.2.0/30`) for traffic engineering!',
      'Capture simultaneously on `P-Core1` and `P-Core2` to observe traffic-engineered separation across the carrier core.',
    ],
    build: () => ({
      format: 'netlab', version: 1, name: 'Carrier P/PE Core Backbone',
      nodes: [
        N(1, 'router', 'PE-West', 250, 290, [
          ip('10.100.1.1', 24), ip('10.200.1.1', 24), ip('172.16.1.1', 30), ip('172.16.2.1', 30),
        ], [
          rt('10.100.2.0', 24, '172.16.1.2', 2),
          rt('10.200.2.0', 24, '172.16.2.2', 3),
        ], { portCount: 4 }),
        N(2, 'router', 'P-Core1', 460, 170, [
          ip('172.16.1.2', 30), ip('172.16.3.1', 30),
        ], [
          rt('10.100.1.0', 24, '172.16.1.1', 0),
          rt('10.200.1.0', 24, '172.16.1.1', 0),
          rt('10.100.2.0', 24, '172.16.3.2', 1),
          rt('10.200.2.0', 24, '172.16.3.2', 1),
        ]),
        N(3, 'router', 'P-Core2', 460, 410, [
          ip('172.16.2.2', 30), ip('172.16.4.1', 30),
        ], [
          rt('10.100.1.0', 24, '172.16.2.1', 0),
          rt('10.200.1.0', 24, '172.16.2.1', 0),
          rt('10.100.2.0', 24, '172.16.4.2', 1),
          rt('10.200.2.0', 24, '172.16.4.2', 1),
        ]),
        N(4, 'router', 'PE-East', 670, 290, [
          ip('10.100.2.1', 24), ip('10.200.2.1', 24), ip('172.16.3.2', 30), ip('172.16.4.2', 30),
        ], [
          rt('10.100.1.0', 24, '172.16.3.1', 2),
          rt('10.200.1.0', 24, '172.16.4.1', 3),
        ], { portCount: 4 }),
        N(5, 'pc', 'CustA-HQ', 80, 170, [ip('10.100.1.10', 24)], gw('10.100.1.1')),
        N(6, 'pc', 'CustB-West', 80, 410, [ip('10.200.1.10', 24)], gw('10.200.1.1')),
        N(7, 'server', 'CustA-DR', 840, 170, [ip('10.100.2.10', 24)], gw('10.100.2.1')),
        N(8, 'server', 'CustB-East', 840, 410, [ip('10.200.2.10', 24)], gw('10.200.2.1')),
      ],
      links: [
        L(5, 0, 1, 0),
        L(6, 0, 1, 1),
        L(1, 2, 2, 0),
        L(1, 3, 3, 0),
        L(2, 1, 4, 2),
        L(3, 1, 4, 3),
        L(4, 0, 7, 0),
        L(4, 1, 8, 0),
      ],
      shapes: [
        S(1, 'rect', 190, 100, 540, 380, '#a58bff', 'Service Provider P / PE Core Network'),
        S(2, 'rect', 20, 105, 140, 130, '#4c9bff', 'Customer A West'),
        S(3, 'rect', 760, 105, 140, 130, '#4c9bff', 'Customer A East'),
        S(4, 'rect', 20, 345, 140, 130, '#3ccf7e', 'Customer B West'),
        S(5, 'rect', 760, 345, 140, 130, '#3ccf7e', 'Customer B East'),
      ],
      notes: [
        { id: 1, x: 310, y: 30, text: 'Traffic-Engineered Carrier Core\n• Customer A (10.100.x.x) transits via Top Core (P-Core1)\n• Customer B (10.200.x.x) transits via Bottom Core (P-Core2)' },
      ],
    }),
  },

  {
    id: 'hyperscale-ai-cluster',
    title: 'AI / GPU Rail-Optimized Training Cluster (Dual-Fabric)',
    level: 'expert',
    category: 'Data Center',
    summary: 'Dual-plane AI cluster where four 4-NIC GPU compute nodes connect simultaneously to a Backend Rail Fabric (Rail-1 & Rail-2 for AllReduce) and a Frontend Storage / OOB Management Fabric.',
    tags: ['AI Cluster', 'Rail-Optimized', '4-NIC GPU Node', 'Dual Fabric', 'NVMe Storage'],
    objectives: [
      'Inspect `GPU-Node1` through `GPU-Node4` — each node has 4 NICs: `eth0` (Rail-1 `10.201.1.0/24`), `eth1` (Rail-2 `10.201.2.0/24`), `eth2` (Storage `10.202.0.0/24`), and `eth3` (OOB Mgmt `192.168.200.0/24`).',
      'Test non-blocking AllReduce rail communication on Rail-1 (`ping 10.201.1.14` from `GPU-Node1`) and cross-rail routing through `R-Rail-Spine` (`ping 10.201.2.14`).',
      'Ping `NVMe-Lustre` (`10.202.0.50`) over the dedicated Storage fabric (`eth2`) and `Slurm-Scheduler` (`192.168.200.10`) over OOB Management (`eth3`).',
    ],
    build: () => ({
      format: 'netlab', version: 1, name: 'AI GPU Rail-Optimized Cluster',
      nodes: [
        N(1, 'router', 'R-Rail-Spine', 460, 80, [
          ip('10.201.1.1', 24), ip('10.201.2.1', 24),
        ]),
        N(2, 'switch', 'SW-Rail1', 280, 190),
        N(3, 'switch', 'SW-Rail2', 640, 190),
        N(4, 'server', 'GPU-Node1', 160, 340, [
          ip('10.201.1.11', 24), ip('10.201.2.11', 24), ip('10.202.0.11', 24), ip('192.168.200.11', 24),
        ], gw('10.201.1.1', 0), { portCount: 4 }),
        N(5, 'server', 'GPU-Node2', 360, 340, [
          ip('10.201.1.12', 24), ip('10.201.2.12', 24), ip('10.202.0.12', 24), ip('192.168.200.12', 24),
        ], gw('10.201.1.1', 0), { portCount: 4 }),
        N(6, 'server', 'GPU-Node3', 560, 340, [
          ip('10.201.1.13', 24), ip('10.201.2.13', 24), ip('10.202.0.13', 24), ip('192.168.200.13', 24),
        ], gw('10.201.1.1', 0), { portCount: 4 }),
        N(7, 'server', 'GPU-Node4', 760, 340, [
          ip('10.201.1.14', 24), ip('10.201.2.14', 24), ip('10.202.0.14', 24), ip('192.168.200.14', 24),
        ], gw('10.201.1.1', 0), { portCount: 4 }),
        N(8, 'switch', 'SW-Storage', 300, 500),
        N(9, 'switch', 'SW-OOB-Mgmt', 620, 500),
        N(10, 'server', 'NVMe-Lustre', 140, 560, [ip('10.202.0.50', 24)]),
        N(11, 'pc', 'Slurm-Master', 780, 560, [ip('192.168.200.10', 24)]),
      ],
      links: [
        L(1, 0, 2, 0),
        L(1, 1, 3, 0),
        L(2, 1, 4, 0), L(2, 2, 5, 0), L(2, 3, 6, 0), L(2, 4, 7, 0),
        L(3, 1, 4, 1), L(3, 2, 5, 1), L(3, 3, 6, 1), L(3, 4, 7, 1),
        L(8, 1, 4, 2), L(8, 2, 5, 2), L(8, 3, 6, 2), L(8, 4, 7, 2), L(8, 5, 10, 0),
        L(9, 1, 4, 3), L(9, 2, 5, 3), L(9, 3, 6, 3), L(9, 4, 7, 3), L(9, 5, 11, 0),
      ],
      shapes: [
        S(1, 'rect', 180, 30, 560, 215, '#a58bff', 'Backend GPU Rail Fabric · Rail-1 (10.201.1.0/24) & Rail-2 (10.201.2.0/24)'),
        S(2, 'rect', 90, 285, 750, 120, '#3ccf7e', '4-NIC GPU Compute Nodes (eth0=Rail1, eth1=Rail2, eth2=Storage, eth3=Mgmt)'),
        S(3, 'rect', 80, 450, 770, 165, '#4c9bff', 'Frontend NVMe Storage (10.202.0.0/24) & Slurm OOB Mgmt (192.168.200.0/24)'),
      ],
      notes: [
        { id: 1, x: 20, y: 30, text: 'AI / GPU Rail-Optimized Fabric\n• 4 GPU servers × 4 NICs = 16 fabric links\n• Separates East-West GPU AllReduce from Storage & Slurm Mgmt' },
      ],
    }),
  },

  {
    id: 'cisco-switch-vlan-stp',
    title: 'Cisco Switch: 802.1Q VLAN Trunking, SVIs & PVST+',
    level: 'advanced',
    category: 'Emulated (IOS & v86)',
    summary: 'Dual Cisco IOS managed switches (NM-16ESW / PA-8E) interconnected with redundant 802.1Q trunks running Spanning Tree (PVST+ / IEEE 802.1D) and inter-VLAN routing.',
    tags: ['Cisco Switch', 'NM-16ESW', '802.1Q Trunk', 'PVST+ / STP', 'Inter-VLAN SVI'],
    objectives: [
      'Attach a Cisco c3725 IOS image (or c7200 image) to `CSW-Core1` and `CSW-Core2` and boot both switches.',
      'Create VLAN 10 (Engineering `10.10.10.0/24`) and VLAN 20 (Production `10.10.20.0/24`) in `vlan database`, configure `fa1/0` and `fa1/1` as `switchport mode trunk`, and assign access ports `fa1/2` (`VLAN 10`) and `fa1/3` (`VLAN 20`).',
      'Right-click `fa1/0` to capture real IEEE 802.1Q tagged frames, PVST+ / 802.1D BPDUs, and CDP announcements on the wire.',
    ],
    build: () => ({
      format: 'netlab', version: 1, name: 'Cisco Switch VLANs & PVST+',
      nodes: [
        N(1, 'ios-sw', 'CSW-Core1', 310, 260, [], [], { portCount: 8 }),
        N(2, 'ios-sw', 'CSW-Core2', 610, 260, [], [], { portCount: 8 }),
        N(3, 'pc', 'Eng-PC1', 110, 150, [ip('10.10.10.11', 24)], gw('10.10.10.1')),
        N(4, 'pc', 'Prod-PC1', 110, 380, [ip('10.10.20.11', 24)], gw('10.10.20.1')),
        N(5, 'server', 'Eng-Srv2', 810, 150, [ip('10.10.10.12', 24)], gw('10.10.10.1')),
        N(6, 'server', 'Prod-Srv2', 810, 380, [ip('10.10.20.12', 24)], gw('10.10.20.1')),
      ],
      links: [
        L(1, 0, 2, 0),
        L(1, 1, 2, 1),
        L(1, 2, 3, 0),
        L(1, 3, 4, 0),
        L(2, 2, 5, 0),
        L(2, 3, 6, 0),
      ],
      shapes: [
        S(1, 'rect', 230, 170, 460, 185, '#2fd0c0', 'Dual Cisco IOS Switches · Redundant 802.1Q Trunks (fa1/0 & fa1/1)'),
        S(2, 'rect', 40, 85, 160, 130, '#4c9bff', 'VLAN 10 · 10.10.10.0/24'),
        S(3, 'rect', 725, 85, 160, 130, '#4c9bff', 'VLAN 10 · 10.10.10.0/24'),
        S(4, 'rect', 40, 315, 160, 130, '#3ccf7e', 'VLAN 20 · 10.10.20.0/24'),
        S(5, 'rect', 725, 315, 160, 130, '#3ccf7e', 'VLAN 20 · 10.10.20.0/24'),
      ],
      notes: [
        { id: 1, x: 250, y: 45, text: 'Cisco Catalyst / NM-16ESW Managed Switch Lab\n• fa1/0 & fa1/1: 802.1Q Trunks + Spanning-Tree / Port-Channel\n• fa1/2: switchport access vlan 10 · fa1/3: switchport access vlan 20\n• Configure interface Vlan10 (10.10.10.1/24) & Vlan20 (10.10.20.1/24) for L3 SVI routing' },
      ],
    }),
  },

  {
    id: 'mikrotik-crs-vlan-rstp',
    title: 'MikroTik CRS Switch: Bridge VLAN Filtering & Router-on-a-Stick',
    level: 'advanced',
    category: 'Emulated (IOS & v86)',
    summary: 'MikroTik CRS managed switch running RouterOS Bridge VLAN Filtering and RSTP across 8 Ethernet ports with an upstream MikroTik RouterOS 802.1Q trunk.',
    tags: ['MikroTik CRS', 'Bridge VLAN Filtering', 'RSTP', '802.1Q Trunk'],
    objectives: [
      'Boot `CRS-Switch` and `MT-Router` with a RouterOS v6 x86 image.',
      'On `CRS-Switch`, create an RSTP VLAN-filtering bridge (`/interface bridge add name=br0 vlan-filtering=yes protocol-mode=rstp`), make `ether1` a tagged trunk for VLANs 10 & 20, and assign `ether2..ether3` (PVID 10) and `ether4..ether5` (PVID 20).',
      'On `MT-Router`, create `/interface vlan` sub-interfaces on `ether1` for VLAN 10 (`10.10.0.1/24`) and VLAN 20 (`10.20.0.1/24`) and ping across VLANs.',
    ],
    build: () => ({
      format: 'netlab', version: 1, name: 'MikroTik CRS Bridge VLANs & RSTP',
      nodes: [
        N(1, 'routeros', 'MT-Router', 460, 110, [], [], { portCount: 4 }),
        N(2, 'mikrotik-sw', 'CRS-Switch', 460, 290, [], [], { portCount: 8 }),
        N(3, 'pc', 'VLAN10-PC1', 150, 440, [ip('10.10.0.10', 24)], gw('10.10.0.1')),
        N(4, 'laptop', 'VLAN10-Laptop', 340, 460, [ip('10.10.0.20', 24)], gw('10.10.0.1')),
        N(5, 'pc', 'VLAN20-PC2', 580, 460, [ip('10.20.0.10', 24)], gw('10.20.0.1')),
        N(6, 'server', 'VLAN20-Server', 770, 440, [ip('10.20.0.50', 24)], gw('10.20.0.1')),
      ],
      links: [
        L(1, 0, 2, 0),
        L(2, 1, 3, 0),
        L(2, 2, 4, 0),
        L(2, 3, 5, 0),
        L(2, 4, 6, 0),
      ],
      shapes: [
        S(1, 'rect', 320, 45, 280, 310, '#a58bff', 'MikroTik RouterOS + CRS Managed Switch (802.1Q Trunk on ether1)'),
        S(2, 'rect', 80, 385, 340, 140, '#4c9bff', 'VLAN 10 Access Ports (ether2–3) · 10.10.0.0/24'),
        S(3, 'rect', 510, 385, 340, 140, '#3ccf7e', 'VLAN 20 Access Ports (ether4–5) · 10.20.0.0/24'),
      ],
      notes: [
        { id: 1, x: 40, y: 55, text: 'MikroTik CRS Switch CLI:\n/interface bridge add name=br0 vlan-filtering=yes protocol-mode=rstp\n/interface bridge port\nadd bridge=br0 interface=ether1\nadd bridge=br0 interface=ether2 pvid=10\nadd bridge=br0 interface=ether3 pvid=10\nadd bridge=br0 interface=ether4 pvid=20\nadd bridge=br0 interface=ether5 pvid=20\n/interface bridge vlan\nadd bridge=br0 tagged=ether1 untagged=ether2,ether3 vlan-ids=10\nadd bridge=br0 tagged=ether1 untagged=ether4,ether5 vlan-ids=20' },
      ],
    }),
  },

  {
    id: 'multivendor-managed-switching',
    title: 'Multi-Vendor Switching Ring: Cisco Switch + MikroTik CRS + Open vSwitch',
    level: 'expert',
    category: 'Emulated (IOS & v86)',
    summary: 'Three-vendor Layer-2 switching triangle combining a Cisco IOS Switch, a MikroTik CRS Switch, and an Open vSwitch / Linux Switch with IEEE 802.1Q trunks and Spanning Tree loop prevention.',
    tags: ['Cisco Switch', 'MikroTik CRS', 'Open vSwitch', 'STP Ring', '802.1Q Interop'],
    objectives: [
      'Boot `CSW-1` (Cisco IOS Switch), `CRS-1` (MikroTik CRS Switch), and `OVS-1` (Open vSwitch / Linux VM) in the triangle topology.',
      'Enable IEEE 802.1D / RSTP Spanning Tree on all three switches and watch BPDUs negotiate a loop-free topology on the wire capture.',
      'Configure VLAN 10 across the inter-switch trunks and verify end-to-end ping between `Host-Cisco` (`10.50.10.11`), `Host-MikroTik` (`10.50.10.12`), and `Host-OVS` (`10.50.10.13`).',
    ],
    build: () => ({
      format: 'netlab', version: 1, name: 'Multi-Vendor Managed Switch Ring',
      nodes: [
        N(1, 'ios-sw', 'CSW-1', 460, 130, [], [], { portCount: 8 }),
        N(2, 'mikrotik-sw', 'CRS-1', 260, 370, [], [], { portCount: 8 }),
        N(3, 'ovs-sw', 'OVS-1', 660, 370, [], [], { portCount: 8 }),
        N(4, 'pc', 'Host-Cisco', 460, 30, [ip('10.50.10.11', 24)]),
        N(5, 'pc', 'Host-MikroTik', 90, 450, [ip('10.50.10.12', 24)]),
        N(6, 'server', 'Host-OVS', 830, 450, [ip('10.50.10.13', 24)]),
      ],
      links: [
        L(1, 0, 2, 0),
        L(1, 1, 3, 0),
        L(2, 1, 3, 1),
        L(1, 2, 4, 0),
        L(2, 2, 5, 0),
        L(3, 2, 6, 0),
      ],
      shapes: [
        S(1, 'rect', 190, 85, 540, 355, '#f2b84b', 'Multi-Vendor L2 Switching Ring · IEEE 802.1Q Trunks + Spanning Tree'),
      ],
      notes: [
        { id: 1, x: 30, y: 80, text: 'Multi-Vendor L2 Interop\n• CSW-1: Cisco IOS Switch (NM-16ESW / PA-8E)\n• CRS-1: MikroTik CRS (RouterOS Bridge + RSTP)\n• OVS-1: Open vSwitch / Linux Bridge\n• Capture any trunk to inspect STP BPDUs & 802.1Q tags' },
      ],
    }),
  },

  {
    id: 'vlan-access-trunk',
    title: 'Access VLANs and an 802.1Q Trunk',
    level: 'intermediate',
    category: 'Switching',
    summary: 'Two simulated switches. VLAN 10 reaches across the trunk. VLAN 20 stays on SW1.',
    tags: ['802.1Q', 'Access', 'Trunk', 'VLAN'],
    objectives: [
      'From A (`10.10.10.1`) ping B (`10.10.10.2`). Both access ports are VLAN 10, and the link between the switches is a trunk.',
      'From A ping C (`10.10.20.1`). C is access VLAN 20, so the frame is dropped.',
      'Select SW1 and read the VLAN table: e0 and e1 are access, e2 is a trunk.',
    ],
    build: () => ({
      format: 'netlab', version: 1, name: 'VLAN Access and Trunk',
      nodes: [
        N(1, 'pc', 'A', 80, 160, [ip('10.10.10.1')]),
        N(2, 'pc', 'C', 80, 360, [ip('10.10.20.1')]),
        N(3, 'switch', 'SW1', 300, 260, [{ access_vlan: 10 }, { access_vlan: 20 }, { trunk: true }]),
        N(4, 'switch', 'SW2', 560, 180, [{ trunk: true }, { access_vlan: 10 }]),
        N(5, 'pc', 'B', 780, 180, [ip('10.10.10.2')]),
      ],
      links: [
        L(1, 0, 3, 0),
        L(2, 0, 3, 1),
        L(3, 2, 4, 0),
        L(4, 1, 5, 0),
      ],
      shapes: [
        S(1, 'rect', 40, 90, 220, 160, '#4c9bff', 'VLAN 10'),
        S(2, 'rect', 40, 290, 220, 150, '#3ccf7e', 'VLAN 20'),
        S(3, 'rect', 680, 100, 200, 170, '#4c9bff', 'VLAN 10'),
      ],
      notes: [
        { id: 1, x: 300, y: 40, text: 'Simulated 802.1Q\n• A ping 10.10.10.2 succeeds\n• A ping 10.10.20.1 fails\n• Capture the trunk to see the VLAN 10 tag' },
      ],
    }),
  },

  {
    id: 'dhcp-relay',
    title: 'DHCP Relay Across a Router',
    level: 'intermediate',
    category: 'Services',
    summary: 'A PC with no address reaches a DHCP server on the far side of a router via ip helper-address.',
    tags: ['DHCP', 'Relay', 'Helper'],
    objectives: [
      'Start PC1. Its startup script sends `ip dhcp`.',
      'Watch R1 relay the Discover to Server (`10.2.0.10`) with giaddr `10.1.0.1`.',
      'PC1 should lease `10.1.0.10`. Run `show ip` to confirm.',
    ],
    build: () => ({
      format: 'netlab', version: 1, name: 'DHCP Relay',
      nodes: [
        N(1, 'pc', 'PC1', 120, 240, [], [], { powered: false, startup: 'ip dhcp', startupBoot: true }),
        N(2, 'router', 'R1', 400, 240, [
          { ...ip('10.1.0.1'), helper: '10.2.0.10' },
          ip('10.2.0.1'),
        ]),
        N(3, 'server', 'Server', 680, 240, [ip('10.2.0.10')], gw('10.2.0.1'), { svc: { dhcp: true } }),
      ],
      links: [
        L(1, 0, 2, 0),
        L(2, 1, 3, 0),
      ],
      shapes: [
        S(1, 'rect', 40, 150, 240, 180, '#4c9bff', '10.1.0.0/24'),
        S(2, 'rect', 560, 150, 240, 180, '#f2b84b', '10.2.0.0/24'),
      ],
      notes: [
        { id: 1, x: 250, y: 60, text: 'DHCP relay\n• Start PC1 (Apply on start runs ip dhcp)\n• Or open PC1 and type: ip dhcp\n• Server pool is the giaddr subnet' },
      ],
    }),
  },

  {
    id: 'ospf-two-routers',
    title: 'Two Routers, One OSPF Area',
    level: 'intermediate',
    category: 'Routing',
    summary: 'R1 and R2 learn each other\'s LANs with simplified OSPFv2. A static route of the same length still wins.',
    tags: ['OSPF', 'Area 0', 'Static'],
    objectives: [
      'Select R1 and run `show ip ospf`, then `show ip route`. The 10.2.0.0/24 route is learned (O).',
      'From PC1 ping `10.2.0.10`.',
      'On R1, `ip route 10.2.0.0/24 10.0.0.2` and look again: the static route replaces the OSPF route of the same length.',
    ],
    build: () => ({
      format: 'netlab', version: 1, name: 'Two-Router OSPF',
      nodes: [
        N(1, 'pc', 'PC1', 80, 240, [ip('10.1.0.10')], gw('10.1.0.1')),
        N(2, 'router', 'R1', 320, 240, [ip('10.1.0.1'), ip('10.0.0.1', 30)], [], {
          ospf: { nets: [{ prefix: '10.0.0.0', wild: '0.255.255.255', area: 0 }] },
        }),
        N(3, 'router', 'R2', 560, 240, [ip('10.0.0.2', 30), ip('10.2.0.1')], [], {
          ospf: { nets: [{ prefix: '10.0.0.0', wild: '0.255.255.255', area: 0 }] },
        }),
        N(4, 'pc', 'PC2', 800, 240, [ip('10.2.0.10')], gw('10.2.0.1')),
      ],
      links: [
        L(1, 0, 2, 0),
        L(2, 1, 3, 0),
        L(3, 1, 4, 0),
      ],
      shapes: [
        S(1, 'rect', 40, 150, 200, 180, '#4c9bff', '10.1.0.0/24'),
        S(2, 'rect', 280, 150, 320, 180, '#a58bff', '10.0.0.0/30 · area 0'),
        S(3, 'rect', 680, 150, 220, 180, '#3ccf7e', '10.2.0.0/24'),
      ],
      notes: [
        { id: 1, x: 280, y: 40, text: 'Simplified OSPFv2 (not an IOS packet)\n• PC1: ping 10.2.0.10\n• R1: show ip route\n• Hellos stay on the link' },
      ],
    }),
  },

  {
    id: 'static-nat-forward',
    title: 'Static NAT and a Port Forward',
    level: 'intermediate',
    category: 'NAT',
    summary: 'One outside address. PC1 is a 1:1 map. TCP 80 on that address reaches the inside web server.',
    tags: ['NAT', 'Static', 'Port Forward', 'HTTP'],
    objectives: [
      'From WAN ping `203.0.113.1`. PC1 answers, because the 1:1 map owns that address.',
      'From WAN run `curl 203.0.113.1 80`. The port forward delivers it to Web.',
      'On R1 run `show ip nat` and `show running-config` to see both maps.',
    ],
    build: () => ({
      format: 'netlab', version: 1, name: 'Static NAT and Port Forward',
      nodes: [
        N(1, 'pc', 'PC1', 80, 160, [ip('10.0.0.2')], gw('10.0.0.1')),
        N(2, 'server', 'Web', 80, 360, [ip('10.0.0.10')], gw('10.0.0.1'), { svc: { http: true } }),
        N(3, 'switch', 'SW1', 300, 260),
        N(4, 'router', 'R1', 540, 260, [
          { ...ip('10.0.0.1'), nat: 'inside' },
          { ...ip('203.0.113.1'), nat: 'outside' },
        ], [], {
          static_nat: [
            { inside: '10.0.0.2', proto: 0, inside_port: 0, outside_port: 0 },
            { inside: '10.0.0.10', proto: 6, inside_port: 80, outside_port: 80 },
          ],
        }),
        N(5, 'pc', 'WAN', 800, 260, [ip('203.0.113.2')], gw('203.0.113.1')),
      ],
      links: [
        L(1, 0, 3, 0),
        L(2, 0, 3, 1),
        L(3, 2, 4, 0),
        L(4, 1, 5, 0),
      ],
      shapes: [
        S(1, 'rect', 30, 90, 400, 340, '#4c9bff', 'Inside · 10.0.0.0/24'),
        S(2, 'rect', 700, 160, 200, 200, '#f2b84b', '203.0.113.0/24'),
      ],
      notes: [
        { id: 1, x: 460, y: 40, text: 'Static NAT\n• WAN: ping 203.0.113.1  (PC1 answers)\n• WAN: curl 203.0.113.1 80  (Web answers)\n• 1:1 uses the outside interface address' },
      ],
    }),
  },
];

export const userTemplates = {
  list() {
    try {
      const raw = JSON.parse(localStorage.getItem(USER_TEMPLATES_KEY) ?? '[]');
      return Array.isArray(raw) ? raw : [];
    } catch { return []; }
  },
  saveAll(list) {
    try { localStorage.setItem(USER_TEMPLATES_KEY, JSON.stringify(list)); } catch { /* quota */ }
  },
  add(entry) {
    const list = this.list().filter(x => x.id !== entry.id);
    list.unshift(entry);
    this.saveAll(list);
    return entry;
  },
  remove(id) {
    const list = this.list().filter(x => x.id !== id);
    this.saveAll(list);
  },
};

export function allTemplates() {
  const custom = userTemplates.list().map(t => ({
    ...t,
    custom: true,
    build: () => structuredClone(t.data),
  }));
  return [...BUILTIN_TEMPLATES, ...custom];
}

const NODE_COLOR = {
  router: '#4c9bff',
  ios: '#2fd0c0',
  'ios-sw': '#2fd0c0',
  routeros: '#a58bff',
  'mikrotik-sw': '#a58bff',
  'ovs-sw': '#f2b84b',
  frr: '#3ccf7e',
  firewall: '#ff5d5d',
  x86: '#f2b84b',
  switch: '#62b4ff',
  pc: '#7cc4ff',
  laptop: '#7cc4ff',
  server: '#3ccf7e',
};

/** Generate an inline SVG schematic thumbnail of a topology template */
export function renderTemplatePreviewSVG(data) {
  const nodes = data?.nodes ?? [];
  const links = data?.links ?? [];
  const shapes = data?.shapes ?? [];
  if (!nodes.length) {
    return `<svg viewBox="0 0 280 140" xmlns="http://www.w3.org/2000/svg"></svg>`;
  }
  const xs = [], ys = [];
  for (const n of nodes) { xs.push(n.x - 45, n.x + 45); ys.push(n.y - 45, n.y + 50); }
  for (const s of shapes) { xs.push(s.x, s.x + s.w); ys.push(s.y, s.y + s.h); }
  const x0 = Math.min(...xs), x1 = Math.max(...xs);
  const y0 = Math.min(...ys), y1 = Math.max(...ys);
  const bw = Math.max(120, x1 - x0), bh = Math.max(80, y1 - y0);
  const W = 280, H = 148, pad = 16;
  const sc = Math.min((W - pad * 2) / bw, (H - pad * 2) / bh);
  const ox = (W - bw * sc) / 2 - x0 * sc;
  const oy = (H - bh * sc) / 2 - y0 * sc;
  const tx = x => (x * sc + ox).toFixed(1);
  const ty = y => (y * sc + oy).toFixed(1);
  const byId = new Map(nodes.map(n => [n.id, n]));

  const shapeEls = shapes.map(s => {
    const sx = Math.min(s.x, s.x + s.w), sy = Math.min(s.y, s.y + s.h);
    const sw = Math.abs(s.w) * sc, sh = Math.abs(s.h) * sc;
    const col = s.stroke || '#4c9bff';
    return `<rect x="${tx(sx)}" y="${ty(sy)}" width="${sw.toFixed(1)}" height="${sh.toFixed(1)}" rx="5" fill="${col}" fill-opacity="0.08" stroke="${col}" stroke-opacity="0.35" stroke-width="1"/>`;
  }).join('');

  const linkEls = links.map(l => {
    const a = byId.get(l.a), b = byId.get(l.b);
    if (!a || !b) return '';
    return `<line x1="${tx(a.x)}" y1="${ty(a.y)}" x2="${tx(b.x)}" y2="${ty(b.y)}" stroke="#6b7a92" stroke-opacity="0.75" stroke-width="1.5"/>`;
  }).join('');

  const nodeEls = nodes.map(n => {
    const col = NODE_COLOR[n.tpl] ?? '#7cc4ff';
    const cx = tx(n.x), cy = ty(n.y);
    const isSw = n.tpl === 'switch' || n.tpl === 'ios-sw' || n.tpl === 'mikrotik-sw' || n.tpl === 'ovs-sw';
    const isSrv = n.tpl === 'server' || n.tpl === 'x86';
    const glyph = isSw
      ? `<rect x="${(cx - 8).toFixed(1)}" y="${(cy - 5).toFixed(1)}" width="16" height="10" rx="2.5" fill="#12161d" stroke="${col}" stroke-width="1.8"/>`
      : isSrv
        ? `<rect x="${(cx - 6).toFixed(1)}" y="${(cy - 7).toFixed(1)}" width="12" height="14" rx="2.5" fill="#12161d" stroke="${col}" stroke-width="1.8"/>`
        : `<circle cx="${cx}" cy="${cy}" r="6.5" fill="#12161d" stroke="${col}" stroke-width="1.8"/>`;
    return `${glyph}<circle cx="${cx}" cy="${cy}" r="2.3" fill="${col}"/>`;
  }).join('');

  return `<svg viewBox="0 0 ${W} ${H}" xmlns="http://www.w3.org/2000/svg" aria-hidden="true">${shapeEls}${linkEls}${nodeEls}</svg>`;
}

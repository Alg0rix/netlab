/* tslint:disable */
/* eslint-disable */

export class NetLab {
    free(): void;
    [Symbol.dispose](): void;
    /**
     * `proto` is "ip", "icmp", "tcp" or "udp". Port 0 means any.
     */
    acl_add(dev: number, num: number, permit: boolean, proto: string, src: string, src_wild: string, dst: string, dst_wild: string, sport: number, dport: number): boolean;
    acl_clear(dev: number, num: number): void;
    /**
     * kind: "host" | "switch" | "router" | "ext"
     */
    add_device(kind: string, name: string, x: number, y: number): number;
    add_dns_record(dev: number, name: string, ip: string): boolean;
    /**
     * Returns link id, or -1 if either device has no free port.
     */
    add_link(a: number, b: number): number;
    /**
     * Connect specific ports; -1 if a port is missing or busy.
     */
    add_link_ports(a: number, pa: number, b: number, pb: number): number;
    add_route(dev: number, prefix: string, mask: string, next_hop: string, out_port: number): boolean;
    clear_ip(dev: number, port: number): void;
    dhcp_client(dev: number, port: number): boolean;
    dns_query(dev: number, name: string): boolean;
    /**
     * Captured frames since last call: array of {link, tick, from, bytes}.
     */
    drain_captures(): Array<any>;
    /**
     * JSON array of events since the last call.
     */
    drain_events(): string;
    /**
     * Drain wire bytes bound for emulated nodes.
     * Returns a JS array of {dev, port, bytes: Uint8Array}.
     */
    drain_ext(): Array<any>;
    /**
     * Inject real Ethernet wire bytes from an emulated node into an ext
     * device port.
     */
    ext_inject(dev: number, port: number, bytes: Uint8Array | number[]): void;
    http_get(dev: number, dst: string, port: number): boolean;
    /**
     * Place a raw Ethernet frame onto a link, travelling away from `from`.
     */
    inject_link(link: number, from: number, bytes: Uint8Array | number[]): boolean;
    move_device(id: number, x: number, y: number): void;
    nat_static_clear(dev: number, inside: string, proto: string, outside_port: number): boolean;
    /**
     * `proto` is "ip" (1:1) or "tcp" / "udp" (static port forward).
     */
    nat_static(dev: number, inside: string, proto: string, inside_port: number, outside_port: number): boolean;
    constructor();
    ospf_clear(dev: number): void;
    /**
     * `area` < 0 clears `ip ospf` on that interface.
     */
    ospf_interface(dev: number, port: number, area: number): boolean;
    ospf_network(dev: number, prefix: string, wild: string, area: number): boolean;
    ping(src: number, dst_ip: string): boolean;
    probe_tcp(dev: number, dst: string, port: number): boolean;
    /**
     * ICMP echo with TTL. Used by traceroute.
     */
    probe_ttl(src: number, dst_ip: string, ttl: number): boolean;
    probe_udp(dev: number, dst: string, port: number, payload: string): boolean;
    remove_device(id: number): void;
    remove_link(id: number): void;
    remove_route(dev: number, idx: number): void;
    rename_device(id: number, name: string): void;
    /**
     * `dir` is "in", "out", "off-in" or "off-out".
     */
    set_access_group(dev: number, port: number, num: number, dir: string): boolean;
    set_capture(id: number, on: boolean): void;
    set_dns(dev: number, ip: string): boolean;
    set_gateway(dev: number, gw: string): boolean;
    /**
     * Empty `ip` clears the helper.
     */
    set_helper(dev: number, port: number, ip: string): boolean;
    set_ip(dev: number, port: number, ip: string, mask: string): boolean;
    /**
     * Link impairment. `delay` and `jitter` are ticks (1 ms in Realtime).
     * `loss` is a percent. `mtu` 0 means no limit.
     * `speed` is "10M" | "100M" | "1G" | "10G" | "WAN".
     */
    set_link_impair(id: number, delay: number, jitter: number, loss: number, mtu: number, speed: string): boolean;
    set_link_up(id: number, up: boolean): void;
    /**
     * role: "inside" | "outside" | "off"
     */
    set_nat(dev: number, port: number, role: string): boolean;
    set_power(id: number, on: boolean): void;
    set_services(dev: number, dhcp: boolean, dns: boolean, http: boolean): void;
    /**
     * `mode` is "access" or "trunk". `allowed` is "" / "all" or "1,10,20".
     */
    set_switchport(dev: number, port: number, mode: string, access: number, native: number, allowed: string): boolean;
    /**
     * JSON snapshot of the whole topology for rendering.
     */
    snapshot(): string;
    tick(): void;
}

export type InitInput = RequestInfo | URL | Response | BufferSource | WebAssembly.Module;

export interface InitOutput {
    readonly memory: WebAssembly.Memory;
    readonly __wbg_netlab_free: (a: number, b: number) => void;
    readonly netlab_acl_add: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number, i: number, j: number, k: number, l: number, m: number, n: number, o: number, p: number) => number;
    readonly netlab_acl_clear: (a: number, b: number, c: number) => void;
    readonly netlab_add_device: (a: number, b: number, c: number, d: number, e: number, f: number, g: number) => number;
    readonly netlab_add_dns_record: (a: number, b: number, c: number, d: number, e: number, f: number) => number;
    readonly netlab_add_link: (a: number, b: number, c: number) => number;
    readonly netlab_add_link_ports: (a: number, b: number, c: number, d: number, e: number) => number;
    readonly netlab_add_route: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number, i: number) => number;
    readonly netlab_clear_ip: (a: number, b: number, c: number) => void;
    readonly netlab_dhcp_client: (a: number, b: number, c: number) => number;
    readonly netlab_dns_query: (a: number, b: number, c: number, d: number) => number;
    readonly netlab_drain_captures: (a: number) => any;
    readonly netlab_drain_events: (a: number) => [number, number];
    readonly netlab_drain_ext: (a: number) => any;
    readonly netlab_ext_inject: (a: number, b: number, c: number, d: number, e: number) => void;
    readonly netlab_http_get: (a: number, b: number, c: number, d: number, e: number) => number;
    readonly netlab_inject_link: (a: number, b: number, c: number, d: number, e: number) => number;
    readonly netlab_move_device: (a: number, b: number, c: number, d: number) => void;
    readonly netlab_nat_static: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number) => number;
    readonly netlab_nat_static_clear: (a: number, b: number, c: number, d: number, e: number, f: number, g: number) => number;
    readonly netlab_new: () => number;
    readonly netlab_ospf_clear: (a: number, b: number) => void;
    readonly netlab_ospf_interface: (a: number, b: number, c: number, d: number) => number;
    readonly netlab_ospf_network: (a: number, b: number, c: number, d: number, e: number, f: number, g: number) => number;
    readonly netlab_ping: (a: number, b: number, c: number, d: number) => number;
    readonly netlab_probe_tcp: (a: number, b: number, c: number, d: number, e: number) => number;
    readonly netlab_probe_ttl: (a: number, b: number, c: number, d: number, e: number) => number;
    readonly netlab_probe_udp: (a: number, b: number, c: number, d: number, e: number, f: number, g: number) => number;
    readonly netlab_remove_device: (a: number, b: number) => void;
    readonly netlab_remove_link: (a: number, b: number) => void;
    readonly netlab_remove_route: (a: number, b: number, c: number) => void;
    readonly netlab_rename_device: (a: number, b: number, c: number, d: number) => void;
    readonly netlab_set_access_group: (a: number, b: number, c: number, d: number, e: number, f: number) => number;
    readonly netlab_set_capture: (a: number, b: number, c: number) => void;
    readonly netlab_set_dns: (a: number, b: number, c: number, d: number) => number;
    readonly netlab_set_gateway: (a: number, b: number, c: number, d: number) => number;
    readonly netlab_set_helper: (a: number, b: number, c: number, d: number, e: number) => number;
    readonly netlab_set_ip: (a: number, b: number, c: number, d: number, e: number, f: number, g: number) => number;
    readonly netlab_set_link_impair: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number) => number;
    readonly netlab_set_link_up: (a: number, b: number, c: number) => void;
    readonly netlab_set_nat: (a: number, b: number, c: number, d: number, e: number) => number;
    readonly netlab_set_power: (a: number, b: number, c: number) => void;
    readonly netlab_set_services: (a: number, b: number, c: number, d: number, e: number) => void;
    readonly netlab_set_switchport: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number, i: number) => number;
    readonly netlab_snapshot: (a: number) => [number, number];
    readonly netlab_tick: (a: number) => void;
    readonly __wbindgen_exn_store: (a: number) => void;
    readonly __externref_table_alloc: () => number;
    readonly __wbindgen_externrefs: WebAssembly.Table;
    readonly __wbindgen_malloc: (a: number, b: number) => number;
    readonly __wbindgen_realloc: (a: number, b: number, c: number, d: number) => number;
    readonly __wbindgen_free: (a: number, b: number, c: number) => void;
    readonly __wbindgen_start: () => void;
}

export type SyncInitInput = BufferSource | WebAssembly.Module;

/**
 * Instantiates the given `module`, which can either be bytes or
 * a precompiled `WebAssembly.Module`.
 *
 * @param {{ module: SyncInitInput }} module - Passing `SyncInitInput` directly is deprecated.
 *
 * @returns {InitOutput}
 */
export function initSync(module: { module: SyncInitInput } | SyncInitInput): InitOutput;

/**
 * If `module_or_path` is {RequestInfo} or {URL}, makes a request and
 * for everything else, calls `WebAssembly.instantiate` directly.
 *
 * @param {{ module_or_path: InitInput | Promise<InitInput> }} module_or_path - Passing `InitInput` directly is deprecated.
 *
 * @returns {Promise<InitOutput>}
 */
export default function __wbg_init (module_or_path?: { module_or_path: InitInput | Promise<InitInput> } | InitInput | Promise<InitInput>): Promise<InitOutput>;

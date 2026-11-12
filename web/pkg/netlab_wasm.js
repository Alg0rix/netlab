/* @ts-self-types="./netlab_wasm.d.ts" */

export class NetLab {
    __destroy_into_raw() {
        const ptr = this.__wbg_ptr;
        this.__wbg_ptr = 0;
        NetLabFinalization.unregister(this);
        return ptr;
    }
    free() {
        const ptr = this.__destroy_into_raw();
        wasm.__wbg_netlab_free(ptr, 0);
    }
    /**
     * One ACE as a JS object: `{num, permit, proto, src, src_wild, dst,
     * dst_wild, sport?, dport?}` — `proto` is "ip", "icmp", "tcp" or
     * "udp"; `sport`/`dport` 0 or absent means any.
     * @param {number} dev
     * @param {any} spec
     * @returns {boolean}
     */
    acl_add(dev, spec) {
        const ret = wasm.netlab_acl_add(this.__wbg_ptr, dev, spec);
        return ret !== 0;
    }
    /**
     * @param {number} dev
     * @param {number} num
     */
    acl_clear(dev, num) {
        wasm.netlab_acl_clear(this.__wbg_ptr, dev, num);
    }
    /**
     * kind: "host" | "switch" | "router" | "ext"
     * @param {string} kind
     * @param {string} name
     * @param {number} x
     * @param {number} y
     * @returns {number}
     */
    add_device(kind, name, x, y) {
        const val3 = +x;
        const val4 = +y;
        let ptr0, len0, ptr1, len1;
        try {
            ptr0 = passStringToWasm0(kind, wasm.__wbindgen_export, wasm.__wbindgen_export2);
            len0 = WASM_VECTOR_LEN;
            ptr1 = passStringToWasm0(name, wasm.__wbindgen_export, wasm.__wbindgen_export2);
            len1 = WASM_VECTOR_LEN;
        } catch (e) {
            if (ptr1) { wasm.__wbindgen_export3(ptr1, len1 * 1, 1); }
            if (ptr0) { wasm.__wbindgen_export3(ptr0, len0 * 1, 1); }
            throw e;
        }
        const ret = wasm.netlab_add_device(this.__wbg_ptr, ptr0, len0, ptr1, len1, val3, val4);
        return ret >>> 0;
    }
    /**
     * @param {number} dev
     * @param {string} name
     * @param {string} ip
     * @returns {boolean}
     */
    add_dns_record(dev, name, ip) {
        const val3 = +dev;
        let ptr0, len0, ptr1, len1;
        try {
            ptr0 = passStringToWasm0(name, wasm.__wbindgen_export, wasm.__wbindgen_export2);
            len0 = WASM_VECTOR_LEN;
            ptr1 = passStringToWasm0(ip, wasm.__wbindgen_export, wasm.__wbindgen_export2);
            len1 = WASM_VECTOR_LEN;
        } catch (e) {
            if (ptr1) { wasm.__wbindgen_export3(ptr1, len1 * 1, 1); }
            if (ptr0) { wasm.__wbindgen_export3(ptr0, len0 * 1, 1); }
            throw e;
        }
        const ret = wasm.netlab_add_dns_record(this.__wbg_ptr, val3, ptr0, len0, ptr1, len1);
        return ret !== 0;
    }
    /**
     * Returns link id, or -1 if either device has no free port.
     * @param {number} a
     * @param {number} b
     * @returns {number}
     */
    add_link(a, b) {
        const ret = wasm.netlab_add_link(this.__wbg_ptr, a, b);
        return ret;
    }
    /**
     * Connect specific ports; -1 if a port is missing or busy.
     * @param {number} a
     * @param {number} pa
     * @param {number} b
     * @param {number} pb
     * @returns {number}
     */
    add_link_ports(a, pa, b, pb) {
        const ret = wasm.netlab_add_link_ports(this.__wbg_ptr, a, pa, b, pb);
        return ret;
    }
    /**
     * @param {number} dev
     * @param {string} prefix
     * @param {string} mask
     * @param {string} next_hop
     * @param {number} out_port
     * @returns {boolean}
     */
    add_route(dev, prefix, mask, next_hop, out_port) {
        const val4 = +dev;
        const val5 = +out_port;
        let ptr0, len0, ptr1, len1, ptr2, len2;
        try {
            ptr0 = passStringToWasm0(prefix, wasm.__wbindgen_export, wasm.__wbindgen_export2);
            len0 = WASM_VECTOR_LEN;
            ptr1 = passStringToWasm0(mask, wasm.__wbindgen_export, wasm.__wbindgen_export2);
            len1 = WASM_VECTOR_LEN;
            ptr2 = passStringToWasm0(next_hop, wasm.__wbindgen_export, wasm.__wbindgen_export2);
            len2 = WASM_VECTOR_LEN;
        } catch (e) {
            if (ptr2) { wasm.__wbindgen_export3(ptr2, len2 * 1, 1); }
            if (ptr1) { wasm.__wbindgen_export3(ptr1, len1 * 1, 1); }
            if (ptr0) { wasm.__wbindgen_export3(ptr0, len0 * 1, 1); }
            throw e;
        }
        const ret = wasm.netlab_add_route(this.__wbg_ptr, val4, ptr0, len0, ptr1, len1, ptr2, len2, val5);
        return ret !== 0;
    }
    /**
     * Lab-document seat apply — one grouped call for every codec seat the
     * per-field exports cannot reach: port admin_up/method/stats, static
     * routes with ad/metric/learned_tick, mac/arp rows, vlan seats, toggles,
     * and the ospf block. `fields` is the JSON object labyaml's applySeats()
     * emits; the missing-session Err matches every cli_* export. A rejected
     * seat set returns `netlab: bad device fields` and leaves the device
     * untouched — the apply runs on a scratch clone and commits whole.
     * @param {number} dev
     * @param {string} fields
     */
    apply_device_fields(dev, fields) {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16) >>> 0;
            const val2 = +dev;
            const ptr0 = passStringToWasm0(fields, wasm.__wbindgen_export, wasm.__wbindgen_export2);
            const len0 = WASM_VECTOR_LEN;
            wasm.netlab_apply_device_fields(retptr, this.__wbg_ptr, val2, ptr0, len0);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            if (r1) {
                throw takeFromExternrefTable0(r0);
            }
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
        }
    }
    /**
     * Snapshot the device's current config as its authored baseline —
     * call it once the lab document has finished applying one node.
     * @param {number} dev
     */
    baseline_commit(dev) {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16) >>> 0;
            wasm.netlab_baseline_commit(retptr, this.__wbg_ptr, dev);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            if (r1) {
                throw takeFromExternrefTable0(r0);
            }
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
        }
    }
    /**
     * @param {number} dev
     * @param {number} port
     */
    clear_ip(dev, port) {
        wasm.netlab_clear_ip(this.__wbg_ptr, dev, port);
    }
    /**
     * @param {number} dev
     * @param {string} before_cursor
     * @returns {any}
     */
    cli_complete(dev, before_cursor) {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16) >>> 0;
            const val2 = +dev;
            const ptr0 = passStringToWasm0(before_cursor, wasm.__wbindgen_export, wasm.__wbindgen_export2);
            const len0 = WASM_VECTOR_LEN;
            wasm.netlab_cli_complete(retptr, this.__wbg_ptr, val2, ptr0, len0);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            var r2 = getDataViewMemory0().getInt32(retptr + 4 * 2, true);
            if (r2) {
                throw takeFromExternrefTable0(r1);
            }
            return takeFromExternrefTable0(r0);
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
        }
    }
    /**
     * @param {number} dev
     * @param {string} line
     * @returns {any}
     */
    cli_exec(dev, line) {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16) >>> 0;
            const val2 = +dev;
            const ptr0 = passStringToWasm0(line, wasm.__wbindgen_export, wasm.__wbindgen_export2);
            const len0 = WASM_VECTOR_LEN;
            wasm.netlab_cli_exec(retptr, this.__wbg_ptr, val2, ptr0, len0);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            var r2 = getDataViewMemory0().getInt32(retptr + 4 * 2, true);
            if (r2) {
                throw takeFromExternrefTable0(r1);
            }
            return takeFromExternrefTable0(r0);
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
        }
    }
    /**
     * @param {number} dev
     * @param {string} before_cursor
     * @returns {any}
     */
    cli_help(dev, before_cursor) {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16) >>> 0;
            const val2 = +dev;
            const ptr0 = passStringToWasm0(before_cursor, wasm.__wbindgen_export, wasm.__wbindgen_export2);
            const len0 = WASM_VECTOR_LEN;
            wasm.netlab_cli_help(retptr, this.__wbg_ptr, val2, ptr0, len0);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            var r2 = getDataViewMemory0().getInt32(retptr + 4 * 2, true);
            if (r2) {
                throw takeFromExternrefTable0(r1);
            }
            return takeFromExternrefTable0(r0);
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
        }
    }
    /**
     * @param {number} dev
     * @param {string} op
     * @returns {any}
     */
    cli_key(dev, op) {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16) >>> 0;
            const val2 = +dev;
            const ptr0 = passStringToWasm0(op, wasm.__wbindgen_export, wasm.__wbindgen_export2);
            const len0 = WASM_VECTOR_LEN;
            wasm.netlab_cli_key(retptr, this.__wbg_ptr, val2, ptr0, len0);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            var r2 = getDataViewMemory0().getInt32(retptr + 4 * 2, true);
            if (r2) {
                throw takeFromExternrefTable0(r1);
            }
            return takeFromExternrefTable0(r0);
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
        }
    }
    /**
     * @param {number} dev
     * @returns {string}
     */
    cli_prompt(dev) {
        let deferred2_0;
        let deferred2_1;
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16) >>> 0;
            wasm.netlab_cli_prompt(retptr, this.__wbg_ptr, dev);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            var r2 = getDataViewMemory0().getInt32(retptr + 4 * 2, true);
            var r3 = getDataViewMemory0().getInt32(retptr + 4 * 3, true);
            var ptr1 = r0;
            var len1 = r1;
            if (r3) {
                ptr1 = 0; len1 = 0;
                throw takeFromExternrefTable0(r2);
            }
            deferred2_0 = ptr1;
            deferred2_1 = len1;
            return getStringFromWasm0(ptr1, len1);
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
            wasm.__wbindgen_export3(deferred2_0, deferred2_1, 1);
        }
    }
    /**
     * @param {number} dev
     * @param {string} text
     * @returns {any}
     */
    cli_script(dev, text) {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16) >>> 0;
            const val2 = +dev;
            const ptr0 = passStringToWasm0(text, wasm.__wbindgen_export, wasm.__wbindgen_export2);
            const len0 = WASM_VECTOR_LEN;
            wasm.netlab_cli_script(retptr, this.__wbg_ptr, val2, ptr0, len0);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            var r2 = getDataViewMemory0().getInt32(retptr + 4 * 2, true);
            if (r2) {
                throw takeFromExternrefTable0(r1);
            }
            return takeFromExternrefTable0(r0);
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
        }
    }
    /**
     * @param {number} dev
     * @param {string[]} names
     */
    cli_set_ifnames(dev, names) {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16) >>> 0;
            const val2 = +dev;
            const ptr0 = passArrayJsValueToWasm0(names, wasm.__wbindgen_export);
            const len0 = WASM_VECTOR_LEN;
            wasm.netlab_cli_set_ifnames(retptr, this.__wbg_ptr, val2, ptr0, len0);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            if (r1) {
                throw takeFromExternrefTable0(r0);
            }
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
        }
    }
    /**
     * @param {number} dev
     * @param {string | null} [text]
     */
    cli_set_startup(dev, text) {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16) >>> 0;
            const val2 = +dev;
            var ptr0 = isLikeNone(text) ? 0 : passStringToWasm0(text, wasm.__wbindgen_export, wasm.__wbindgen_export2);
            var len0 = WASM_VECTOR_LEN;
            wasm.netlab_cli_set_startup(retptr, this.__wbg_ptr, val2, ptr0, len0);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            if (r1) {
                throw takeFromExternrefTable0(r0);
            }
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
        }
    }
    /**
     * @param {number} dev
     * @param {number} port
     * @returns {boolean}
     */
    dhcp_client(dev, port) {
        const ret = wasm.netlab_dhcp_client(this.__wbg_ptr, dev, port);
        return ret !== 0;
    }
    /**
     * @param {number} dev
     * @param {string} name
     * @returns {boolean}
     */
    dns_query(dev, name) {
        const val2 = +dev;
        const ptr0 = passStringToWasm0(name, wasm.__wbindgen_export, wasm.__wbindgen_export2);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.netlab_dns_query(this.__wbg_ptr, val2, ptr0, len0);
        return ret !== 0;
    }
    /**
     * Captured frames since last call: array of {link, tick, from, bytes}.
     * @returns {Array<any>}
     */
    drain_captures() {
        const ret = wasm.netlab_drain_captures(this.__wbg_ptr);
        return ret;
    }
    /**
     * JSON array of events since the last call.
     * @returns {string}
     */
    drain_events() {
        let deferred1_0;
        let deferred1_1;
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16) >>> 0;
            wasm.netlab_drain_events(retptr, this.__wbg_ptr);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            deferred1_0 = r0;
            deferred1_1 = r1;
            return getStringFromWasm0(r0, r1);
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
            wasm.__wbindgen_export3(deferred1_0, deferred1_1, 1);
        }
    }
    /**
     * Drain wire bytes bound for emulated nodes.
     * Returns a JS array of {dev, port, bytes: Uint8Array}.
     * @returns {Array<any>}
     */
    drain_ext() {
        const ret = wasm.netlab_drain_ext(this.__wbg_ptr);
        return ret;
    }
    /**
     * JSON array of link-enqueue flow signals since the last call — feeds the
     * canvas packet trails. Not console output; never parsed for wording.
     * @returns {string}
     */
    drain_flow() {
        let deferred1_0;
        let deferred1_1;
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16) >>> 0;
            wasm.netlab_drain_flow(retptr, this.__wbg_ptr);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            deferred1_0 = r0;
            deferred1_1 = r1;
            return getStringFromWasm0(r0, r1);
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
            wasm.__wbindgen_export3(deferred1_0, deferred1_1, 1);
        }
    }
    /**
     * Inject real Ethernet wire bytes from an emulated node into an ext
     * device port.
     * @param {number} dev
     * @param {number} port
     * @param {Uint8Array | number[]} bytes
     */
    ext_inject(dev, port, bytes) {
        const val2 = +dev;
        const val3 = +port;
        const ptr0 = passArray8ToWasm0(bytes, wasm.__wbindgen_export);
        const len0 = WASM_VECTOR_LEN;
        wasm.netlab_ext_inject(this.__wbg_ptr, val2, val3, ptr0, len0);
    }
    /**
     * @param {number} dev
     * @param {string} dst
     * @param {number} port
     * @returns {boolean}
     */
    http_get(dev, dst, port) {
        const val2 = +dev;
        const val3 = +port;
        const ptr0 = passStringToWasm0(dst, wasm.__wbindgen_export, wasm.__wbindgen_export2);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.netlab_http_get(this.__wbg_ptr, val2, ptr0, len0, val3);
        return ret !== 0;
    }
    /**
     * Place a raw Ethernet frame onto a link, travelling away from `from`.
     * @param {number} link
     * @param {number} from
     * @param {Uint8Array | number[]} bytes
     * @returns {boolean}
     */
    inject_link(link, from, bytes) {
        const val2 = +link;
        const val3 = +from;
        const ptr0 = passArray8ToWasm0(bytes, wasm.__wbindgen_export);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.netlab_inject_link(this.__wbg_ptr, val2, val3, ptr0, len0);
        return ret !== 0;
    }
    /**
     * @param {number} id
     * @param {number} x
     * @param {number} y
     */
    move_device(id, x, y) {
        wasm.netlab_move_device(this.__wbg_ptr, id, x, y);
    }
    /**
     * @param {number} dev
     * @param {string} inside
     * @param {string} proto
     * @param {number} outside_port
     * @returns {boolean}
     */
    nat_static_clear(dev, inside, proto, outside_port) {
        const val3 = +dev;
        const val4 = +outside_port;
        let ptr0, len0, ptr1, len1;
        try {
            ptr0 = passStringToWasm0(inside, wasm.__wbindgen_export, wasm.__wbindgen_export2);
            len0 = WASM_VECTOR_LEN;
            ptr1 = passStringToWasm0(proto, wasm.__wbindgen_export, wasm.__wbindgen_export2);
            len1 = WASM_VECTOR_LEN;
        } catch (e) {
            if (ptr1) { wasm.__wbindgen_export3(ptr1, len1 * 1, 1); }
            if (ptr0) { wasm.__wbindgen_export3(ptr0, len0 * 1, 1); }
            throw e;
        }
        const ret = wasm.netlab_nat_static_clear(this.__wbg_ptr, val3, ptr0, len0, ptr1, len1, val4);
        return ret !== 0;
    }
    /**
     * `proto` is "ip" (1:1) or "tcp" / "udp" (static port forward).
     * @param {number} dev
     * @param {string} inside
     * @param {string} proto
     * @param {number} inside_port
     * @param {number} outside_port
     * @returns {boolean}
     */
    nat_static(dev, inside, proto, inside_port, outside_port) {
        const val3 = +dev;
        const val4 = +inside_port;
        const val5 = +outside_port;
        let ptr0, len0, ptr1, len1;
        try {
            ptr0 = passStringToWasm0(inside, wasm.__wbindgen_export, wasm.__wbindgen_export2);
            len0 = WASM_VECTOR_LEN;
            ptr1 = passStringToWasm0(proto, wasm.__wbindgen_export, wasm.__wbindgen_export2);
            len1 = WASM_VECTOR_LEN;
        } catch (e) {
            if (ptr1) { wasm.__wbindgen_export3(ptr1, len1 * 1, 1); }
            if (ptr0) { wasm.__wbindgen_export3(ptr0, len0 * 1, 1); }
            throw e;
        }
        const ret = wasm.netlab_nat_static(this.__wbg_ptr, val3, ptr0, len0, ptr1, len1, val4, val5);
        return ret !== 0;
    }
    constructor() {
        const ret = wasm.netlab_new();
        this.__wbg_ptr = ret;
        NetLabFinalization.register(this, this.__wbg_ptr, this);
        return this;
    }
    /**
     * @param {number} dev
     */
    ospf_clear(dev) {
        wasm.netlab_ospf_clear(this.__wbg_ptr, dev);
    }
    /**
     * `area` < 0 clears `ip ospf` on that interface.
     * @param {number} dev
     * @param {number} port
     * @param {number} area
     * @returns {boolean}
     */
    ospf_interface(dev, port, area) {
        const ret = wasm.netlab_ospf_interface(this.__wbg_ptr, dev, port, area);
        return ret !== 0;
    }
    /**
     * @param {number} dev
     * @param {string} prefix
     * @param {string} wild
     * @param {number} area
     * @returns {boolean}
     */
    ospf_network(dev, prefix, wild, area) {
        const val3 = +dev;
        const val4 = +area;
        let ptr0, len0, ptr1, len1;
        try {
            ptr0 = passStringToWasm0(prefix, wasm.__wbindgen_export, wasm.__wbindgen_export2);
            len0 = WASM_VECTOR_LEN;
            ptr1 = passStringToWasm0(wild, wasm.__wbindgen_export, wasm.__wbindgen_export2);
            len1 = WASM_VECTOR_LEN;
        } catch (e) {
            if (ptr1) { wasm.__wbindgen_export3(ptr1, len1 * 1, 1); }
            if (ptr0) { wasm.__wbindgen_export3(ptr0, len0 * 1, 1); }
            throw e;
        }
        const ret = wasm.netlab_ospf_network(this.__wbg_ptr, val3, ptr0, len0, ptr1, len1, val4);
        return ret !== 0;
    }
    /**
     * @param {number} src
     * @param {string} dst_ip
     * @returns {boolean}
     */
    ping(src, dst_ip) {
        const val2 = +src;
        const ptr0 = passStringToWasm0(dst_ip, wasm.__wbindgen_export, wasm.__wbindgen_export2);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.netlab_ping(this.__wbg_ptr, val2, ptr0, len0);
        return ret !== 0;
    }
    /**
     * @param {number} dev
     * @param {string} dst
     * @param {number} port
     * @returns {boolean}
     */
    probe_tcp(dev, dst, port) {
        const val2 = +dev;
        const val3 = +port;
        const ptr0 = passStringToWasm0(dst, wasm.__wbindgen_export, wasm.__wbindgen_export2);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.netlab_probe_tcp(this.__wbg_ptr, val2, ptr0, len0, val3);
        return ret !== 0;
    }
    /**
     * ICMP echo with TTL. Used by traceroute.
     * @param {number} src
     * @param {string} dst_ip
     * @param {number} ttl
     * @returns {boolean}
     */
    probe_ttl(src, dst_ip, ttl) {
        const val2 = +src;
        const val3 = +ttl;
        const ptr0 = passStringToWasm0(dst_ip, wasm.__wbindgen_export, wasm.__wbindgen_export2);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.netlab_probe_ttl(this.__wbg_ptr, val2, ptr0, len0, val3);
        return ret !== 0;
    }
    /**
     * @param {number} dev
     * @param {string} dst
     * @param {number} port
     * @param {string} payload
     * @returns {boolean}
     */
    probe_udp(dev, dst, port, payload) {
        const val3 = +dev;
        const val4 = +port;
        let ptr0, len0, ptr1, len1;
        try {
            ptr0 = passStringToWasm0(dst, wasm.__wbindgen_export, wasm.__wbindgen_export2);
            len0 = WASM_VECTOR_LEN;
            ptr1 = passStringToWasm0(payload, wasm.__wbindgen_export, wasm.__wbindgen_export2);
            len1 = WASM_VECTOR_LEN;
        } catch (e) {
            if (ptr1) { wasm.__wbindgen_export3(ptr1, len1 * 1, 1); }
            if (ptr0) { wasm.__wbindgen_export3(ptr0, len0 * 1, 1); }
            throw e;
        }
        const ret = wasm.netlab_probe_udp(this.__wbg_ptr, val3, ptr0, len0, val4, ptr1, len1);
        return ret !== 0;
    }
    /**
     * @param {number} id
     */
    remove_device(id) {
        wasm.netlab_remove_device(this.__wbg_ptr, id);
    }
    /**
     * @param {number} id
     */
    remove_link(id) {
        wasm.netlab_remove_link(this.__wbg_ptr, id);
    }
    /**
     * @param {number} dev
     * @param {number} idx
     */
    remove_route(dev, idx) {
        wasm.netlab_remove_route(this.__wbg_ptr, dev, idx);
    }
    /**
     * @param {number} id
     * @param {string} name
     */
    rename_device(id, name) {
        const val2 = +id;
        const ptr0 = passStringToWasm0(name, wasm.__wbindgen_export, wasm.__wbindgen_export2);
        const len0 = WASM_VECTOR_LEN;
        wasm.netlab_rename_device(this.__wbg_ptr, val2, ptr0, len0);
    }
    /**
     * Reload replay target: the device returns to its authored baseline
     * — the snapshot `baseline_commit` last took. Learned state clears,
     * carrier bindings stay live, and the session seat returns to Exec
     * with an empty context and no dialog.
     * @param {number} dev
     */
    reset_config(dev) {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16) >>> 0;
            wasm.netlab_reset_config(retptr, this.__wbg_ptr, dev);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            if (r1) {
                throw takeFromExternrefTable0(r0);
            }
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
        }
    }
    /**
     * Wholesale lab reset — frees every session; in-flight adapter
     * references degrade to the missing-session Err.
     */
    restore() {
        wasm.netlab_restore(this.__wbg_ptr);
    }
    /**
     * `dir` is "in", "out", "off-in" or "off-out".
     * @param {number} dev
     * @param {number} port
     * @param {number} num
     * @param {string} dir
     * @returns {boolean}
     */
    set_access_group(dev, port, num, dir) {
        const val2 = +dev;
        const val3 = +port;
        const val4 = +num;
        const ptr0 = passStringToWasm0(dir, wasm.__wbindgen_export, wasm.__wbindgen_export2);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.netlab_set_access_group(this.__wbg_ptr, val2, val3, val4, ptr0, len0);
        return ret !== 0;
    }
    /**
     * @param {number} id
     * @param {boolean} on
     */
    set_capture(id, on) {
        wasm.netlab_set_capture(this.__wbg_ptr, id, on);
    }
    /**
     * @param {number} dev
     * @param {string} ip
     * @returns {boolean}
     */
    set_dns(dev, ip) {
        const val2 = +dev;
        const ptr0 = passStringToWasm0(ip, wasm.__wbindgen_export, wasm.__wbindgen_export2);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.netlab_set_dns(this.__wbg_ptr, val2, ptr0, len0);
        return ret !== 0;
    }
    /**
     * @param {number} dev
     * @param {string} gw
     * @returns {boolean}
     */
    set_gateway(dev, gw) {
        const val2 = +dev;
        const ptr0 = passStringToWasm0(gw, wasm.__wbindgen_export, wasm.__wbindgen_export2);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.netlab_set_gateway(this.__wbg_ptr, val2, ptr0, len0);
        return ret !== 0;
    }
    /**
     * Empty `ip` clears the helper.
     * @param {number} dev
     * @param {number} port
     * @param {string} ip
     * @returns {boolean}
     */
    set_helper(dev, port, ip) {
        const val2 = +dev;
        const val3 = +port;
        const ptr0 = passStringToWasm0(ip, wasm.__wbindgen_export, wasm.__wbindgen_export2);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.netlab_set_helper(this.__wbg_ptr, val2, val3, ptr0, len0);
        return ret !== 0;
    }
    /**
     * @param {number} dev
     * @param {number} port
     * @param {string} ip
     * @param {string} mask
     * @returns {boolean}
     */
    set_ip(dev, port, ip, mask) {
        const val3 = +dev;
        const val4 = +port;
        let ptr0, len0, ptr1, len1;
        try {
            ptr0 = passStringToWasm0(ip, wasm.__wbindgen_export, wasm.__wbindgen_export2);
            len0 = WASM_VECTOR_LEN;
            ptr1 = passStringToWasm0(mask, wasm.__wbindgen_export, wasm.__wbindgen_export2);
            len1 = WASM_VECTOR_LEN;
        } catch (e) {
            if (ptr1) { wasm.__wbindgen_export3(ptr1, len1 * 1, 1); }
            if (ptr0) { wasm.__wbindgen_export3(ptr0, len0 * 1, 1); }
            throw e;
        }
        const ret = wasm.netlab_set_ip(this.__wbg_ptr, val3, val4, ptr0, len0, ptr1, len1);
        return ret !== 0;
    }
    /**
     * Link impairment. `delay` and `jitter` are ticks (1 ms in Realtime).
     * `loss` is a percent. `mtu` 0 means no limit.
     * `speed` is "10M" | "100M" | "1G" | "10G" | "WAN".
     * @param {number} id
     * @param {number} delay
     * @param {number} jitter
     * @param {number} loss
     * @param {number} mtu
     * @param {string} speed
     * @returns {boolean}
     */
    set_link_impair(id, delay, jitter, loss, mtu, speed) {
        const val2 = +id;
        const val3 = +delay;
        const val4 = +jitter;
        const val5 = +loss;
        const val6 = +mtu;
        const ptr0 = passStringToWasm0(speed, wasm.__wbindgen_export, wasm.__wbindgen_export2);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.netlab_set_link_impair(this.__wbg_ptr, val2, val3, val4, val5, val6, ptr0, len0);
        return ret !== 0;
    }
    /**
     * @param {number} id
     * @param {boolean} up
     */
    set_link_up(id, up) {
        wasm.netlab_set_link_up(this.__wbg_ptr, id, up);
    }
    /**
     * role: "inside" | "outside" | "off"
     * @param {number} dev
     * @param {number} port
     * @param {string} role
     * @returns {boolean}
     */
    set_nat(dev, port, role) {
        const val2 = +dev;
        const val3 = +port;
        const ptr0 = passStringToWasm0(role, wasm.__wbindgen_export, wasm.__wbindgen_export2);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.netlab_set_nat(this.__wbg_ptr, val2, val3, ptr0, len0);
        return ret !== 0;
    }
    /**
     * @param {number} id
     * @param {boolean} on
     */
    set_power(id, on) {
        wasm.netlab_set_power(this.__wbg_ptr, id, on);
    }
    /**
     * Host pacing hint — realtime ticks are wall milliseconds, simulation
     * ticks are stepped. Probes space their sends to match.
     * @param {boolean} rt
     */
    set_realtime(rt) {
        wasm.netlab_set_realtime(this.__wbg_ptr, rt);
    }
    /**
     * @param {number} dev
     * @param {boolean} dhcp
     * @param {boolean} dns
     * @param {boolean} http
     */
    set_services(dev, dhcp, dns, http) {
        wasm.netlab_set_services(this.__wbg_ptr, dev, dhcp, dns, http);
    }
    /**
     * `mode` is "access" or "trunk". `allowed` is "" / "all" or "1,10,20".
     * @param {number} dev
     * @param {number} port
     * @param {string} mode
     * @param {number} access
     * @param {number} native
     * @param {string} allowed
     * @returns {boolean}
     */
    set_switchport(dev, port, mode, access, native, allowed) {
        const val3 = +dev;
        const val4 = +port;
        const val5 = +access;
        const val6 = +native;
        let ptr0, len0, ptr1, len1;
        try {
            ptr0 = passStringToWasm0(mode, wasm.__wbindgen_export, wasm.__wbindgen_export2);
            len0 = WASM_VECTOR_LEN;
            ptr1 = passStringToWasm0(allowed, wasm.__wbindgen_export, wasm.__wbindgen_export2);
            len1 = WASM_VECTOR_LEN;
        } catch (e) {
            if (ptr1) { wasm.__wbindgen_export3(ptr1, len1 * 1, 1); }
            if (ptr0) { wasm.__wbindgen_export3(ptr0, len0 * 1, 1); }
            throw e;
        }
        const ret = wasm.netlab_set_switchport(this.__wbg_ptr, val3, val4, ptr0, len0, val5, val6, ptr1, len1);
        return ret !== 0;
    }
    /**
     * JSON snapshot of the whole topology for rendering.
     * @returns {string}
     */
    snapshot() {
        let deferred1_0;
        let deferred1_1;
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16) >>> 0;
            wasm.netlab_snapshot(retptr, this.__wbg_ptr);
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            deferred1_0 = r0;
            deferred1_1 = r1;
            return getStringFromWasm0(r0, r1);
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
            wasm.__wbindgen_export3(deferred1_0, deferred1_1, 1);
        }
    }
    tick() {
        wasm.netlab_tick(this.__wbg_ptr);
    }
    /**
     * Arm the traceroute job — the `trace` intent's probe.
     * @param {number} src
     * @param {string} dst_ip
     * @returns {boolean}
     */
    traceroute(src, dst_ip) {
        const val2 = +src;
        const ptr0 = passStringToWasm0(dst_ip, wasm.__wbindgen_export, wasm.__wbindgen_export2);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.netlab_traceroute(this.__wbg_ptr, val2, ptr0, len0);
        return ret !== 0;
    }
}
if (Symbol.dispose) NetLab.prototype[Symbol.dispose] = NetLab.prototype.free;
function __wbg_get_imports() {
    const import0 = {
        __proto__: null,
        __wbg___wbindgen_string_get_7ce8eee7fd327545: function(arg0, arg1) {
            const obj = arg1;
            const ret = typeof(obj) === 'string' ? obj : undefined;
            var ptr1 = isLikeNone(ret) ? 0 : passStringToWasm0(ret, wasm.__wbindgen_export, wasm.__wbindgen_export2);
            var len1 = WASM_VECTOR_LEN;
            getDataViewMemory0().setInt32((arg0 >>> 0) + 4 * 1, len1, true);
            getDataViewMemory0().setInt32((arg0 >>> 0) + 4 * 0, ptr1, true);
        },
        __wbg___wbindgen_throw_4c88d8bf264fd984: function(arg0, arg1) {
            throw new Error(getStringFromWasm0(arg0, arg1));
        },
        __wbg_new_37eb45aaf5eeb820: function() {
            const ret = new Array();
            return ret;
        },
        __wbg_new_cd4348290afae586: function() {
            const ret = new Object();
            return ret;
        },
        __wbg_new_from_slice_b8daf545a41c6aa0: function(arg0, arg1) {
            const ret = new Uint8Array(getArrayU8FromWasm0(arg0, arg1));
            return ret;
        },
        __wbg_parse_c3f72b5a24f83c01: function() { return handleError(function (arg0, arg1) {
            const ret = JSON.parse(getStringFromWasm0(arg0, arg1));
            return ret;
        }, arguments); },
        __wbg_push_c9f4893fc209f768: function(arg0, arg1) {
            const ret = arg0.push(arg1);
            return ret;
        },
        __wbg_set_2793479888329cb6: function() { return handleError(function (arg0, arg1, arg2) {
            const ret = Reflect.set(arg0, arg1, arg2);
            return ret;
        }, arguments); },
        __wbg_stringify_0a0fbf2ef722380a: function() { return handleError(function (arg0) {
            const ret = JSON.stringify(arg0);
            return ret;
        }, arguments); },
        __wbindgen_generic_0000000000000001: function(arg0) {
            // Cast intrinsic for `F64 -> Externref`.
            const ret = arg0;
            return ret;
        },
        __wbindgen_generic_0000000000000002: function(arg0, arg1) {
            // Cast intrinsic for `Ref(String) -> Externref`.
            const ret = getStringFromWasm0(arg0, arg1);
            return ret;
        },
        __wbindgen_init_externref_table: function() {
            const table = wasm.__wbindgen_externrefs;
            table.set(0, undefined);
            table.set(1024, undefined);
            table.set(1025, null);
            table.set(1026, true);
            table.set(1027, false);
        },
    };
    return {
        __proto__: null,
        "./netlab_wasm_bg.js": import0,
    };
}

const NetLabFinalization = (typeof FinalizationRegistry === 'undefined')
    ? { register: () => {}, unregister: () => {} }
    : new FinalizationRegistry(ptr => wasm.__wbg_netlab_free(ptr, 1));

function addToExternrefTable0(obj) {
    const idx = wasm.__wbindgen_export5();
    wasm.__wbindgen_externrefs.set(idx, obj);
    return idx;
}

function getArrayU8FromWasm0(ptr, len) {
    ptr = ptr >>> 0;
    return getUint8ArrayMemory0().subarray(ptr / 1, ptr / 1 + len);
}

let cachedDataViewMemory0 = null;
function getDataViewMemory0() {
    if (cachedDataViewMemory0 === null || cachedDataViewMemory0.buffer.detached === true || (cachedDataViewMemory0.buffer.detached === undefined && cachedDataViewMemory0.buffer !== wasm.memory.buffer)) {
        cachedDataViewMemory0 = new DataView(wasm.memory.buffer);
    }
    return cachedDataViewMemory0;
}

function getStringFromWasm0(ptr, len) {
    ptr = ptr >>> 0;
    return decodeText(getUint8ArrayMemory0().subarray(ptr, ptr + len));
}

let cachedUint8ArrayMemory0 = null;
function getUint8ArrayMemory0() {
    if (cachedUint8ArrayMemory0 === null || cachedUint8ArrayMemory0.byteLength === 0) {
        cachedUint8ArrayMemory0 = new Uint8Array(wasm.memory.buffer);
    }
    return cachedUint8ArrayMemory0;
}

function handleError(f, args) {
    try {
        return f.apply(this, args);
    } catch (e) {
        const idx = addToExternrefTable0(e);
        wasm.__wbindgen_export4(idx);
    }
}

function isLikeNone(x) {
    return x === undefined || x === null;
}

function passArray8ToWasm0(arg, malloc) {
    const ptr = malloc(arg.length * 1, 1) >>> 0;
    getUint8ArrayMemory0().set(arg, ptr / 1);
    WASM_VECTOR_LEN = arg.length;
    return ptr;
}

function passArrayJsValueToWasm0(array, malloc) {
    const ptr = malloc(array.length * 4, 4) >>> 0;
    for (let i = 0; i < array.length; i++) {
        const add = addToExternrefTable0(array[i]);
        getDataViewMemory0().setUint32(ptr + 4 * i, add, true);
    }
    WASM_VECTOR_LEN = array.length;
    return ptr;
}

function passStringToWasm0(arg, malloc, realloc) {
    if (realloc === undefined) {
        const buf = cachedTextEncoder.encode(arg);
        const ptr = malloc(buf.length, 1) >>> 0;
        getUint8ArrayMemory0().subarray(ptr, ptr + buf.length).set(buf);
        WASM_VECTOR_LEN = buf.length;
        return ptr;
    }

    let len = arg.length;
    let ptr = malloc(len, 1) >>> 0;

    const mem = getUint8ArrayMemory0();

    let offset = 0;

    for (; offset < len; offset++) {
        const code = arg.charCodeAt(offset);
        if (code > 0x7F) break;
        mem[ptr + offset] = code;
    }
    if (offset !== len) {
        if (offset !== 0) {
            arg = arg.slice(offset);
        }
        ptr = realloc(ptr, len, len = offset + arg.length * 3, 1) >>> 0;
        const view = getUint8ArrayMemory0().subarray(ptr + offset, ptr + len);
        const ret = cachedTextEncoder.encodeInto(arg, view);

        offset += ret.written;
        ptr = realloc(ptr, len, offset, 1) >>> 0;
    }

    WASM_VECTOR_LEN = offset;
    return ptr;
}

function takeFromExternrefTable0(idx) {
    const value = wasm.__wbindgen_externrefs.get(idx);
    wasm.__wbindgen_export6(idx);
    return value;
}

let cachedTextDecoder = new TextDecoder('utf-8', { ignoreBOM: true });
cachedTextDecoder.decode();
const MAX_SAFARI_DECODE_BYTES = 2146435072;
let numBytesDecoded = 0;
function decodeText(bytes) {
    numBytesDecoded += bytes.length;
    if (numBytesDecoded >= MAX_SAFARI_DECODE_BYTES) {
        cachedTextDecoder = new TextDecoder('utf-8', { ignoreBOM: true });
        cachedTextDecoder.decode();
        numBytesDecoded = bytes.length;
    }
    return cachedTextDecoder.decode(bytes);
}

const cachedTextEncoder = new TextEncoder();

if (!('encodeInto' in cachedTextEncoder)) {
    cachedTextEncoder.encodeInto = function (arg, view) {
        const buf = cachedTextEncoder.encode(arg);
        view.set(buf);
        return {
            read: arg.length,
            written: buf.length
        };
    };
}

let WASM_VECTOR_LEN = 0;

let wasmModule, wasmInstance, wasm;
function __wbg_finalize_init(instance, module) {
    wasmInstance = instance;
    wasm = instance.exports;
    wasmModule = module;
    cachedDataViewMemory0 = null;
    cachedUint8ArrayMemory0 = null;
    wasm.__wbindgen_start();
    return wasm;
}

async function __wbg_load(module, imports) {
    if (typeof Response === 'function' && module instanceof Response) {
        if (!module.ok) {
            throw new Error(`failed to fetch Wasm: ${module.status} ${module.statusText} fetching '${module.url}'`);
        }

        if (typeof WebAssembly.instantiateStreaming === 'function') {
            try {
                return await WebAssembly.instantiateStreaming(module, imports);
            } catch (e) {
                const validResponse = expectedResponseType(module.type);

                if (validResponse && module.headers.get('Content-Type') !== 'application/wasm') {
                    console.warn("`WebAssembly.instantiateStreaming` failed because your server does not serve Wasm with `application/wasm` MIME type. Falling back to `WebAssembly.instantiate` which is slower. Original error:\n", e);

                } else { throw e; }
            }
        }

        const bytes = await module.arrayBuffer();
        return await WebAssembly.instantiate(bytes, imports);
    } else {
        const instance = await WebAssembly.instantiate(module, imports);

        if (instance instanceof WebAssembly.Instance) {
            return { instance, module };
        } else {
            return instance;
        }
    }

    function expectedResponseType(type) {
        switch (type) {
            case 'basic': case 'cors': case 'default': return true;
        }
        return false;
    }
}

function initSync(module) {
    if (wasm !== undefined) return wasm;


    if (module !== undefined) {
        if (Object.getPrototypeOf(module) === Object.prototype) {
            ({module} = module)
        } else {
            console.warn('using deprecated parameters for `initSync()`; pass a single object instead')
        }
    }

    const imports = __wbg_get_imports();
    if (!(module instanceof WebAssembly.Module)) {
        module = new WebAssembly.Module(module);
    }
    const instance = new WebAssembly.Instance(module, imports);
    return __wbg_finalize_init(instance, module);
}

let __wbg_init_promise;
async function __wbg_init(module_or_path) {
    if (wasm !== undefined) return wasm;
    if (__wbg_init_promise === undefined) {
        __wbg_init_promise = __wbg_init_once(module_or_path);
        __wbg_init_promise.catch(() => { __wbg_init_promise = undefined; });
    }
    return __wbg_init_promise;
}

async function __wbg_init_once(module_or_path) {

    if (module_or_path !== undefined) {
        if (Object.getPrototypeOf(module_or_path) === Object.prototype) {
            ({module_or_path} = module_or_path)
        } else {
            console.warn('using deprecated parameters for the initialization function; pass a single object instead')
        }
    }

    if (module_or_path === undefined) {
        module_or_path = new URL('netlab_wasm_bg.wasm', import.meta.url);
    }
    const imports = __wbg_get_imports();

    if (typeof module_or_path === 'string' || (typeof Request === 'function' && module_or_path instanceof Request) || (typeof URL === 'function' && module_or_path instanceof URL)) {
        module_or_path = fetch(module_or_path);
    }

    const { instance, module } = await __wbg_load(await module_or_path, imports);

    return __wbg_finalize_init(instance, module);
}

export { initSync, __wbg_init as default };

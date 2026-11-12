//! User-triggered actions: ping and L4/L7 service probes.

use crate::frame::{Frame, Payload};
use crate::parse_ip;
use crate::DeviceId;

use super::Topology;

impl Topology {
    /// Arm the five-probe ping block on `src` (C4 header now, C3 glyphs and
    /// the C5 summary land on the event queue as the job runs).
    pub fn ping(&mut self, src: DeviceId, dst_ip: &str) -> bool {
        let Some(dst) = parse_ip(dst_ip) else {
            return false;
        };
        let tick = self.tick;
        let Some(d) = self.devices.get_mut(&src) else {
            return false;
        };
        if !d.powered {
            return false;
        }
        d.probe_ping(dst, tick, &mut self.events);
        true
    }

    /// Arm the traceroute job on `src` (C6 header now, C7/C8 rows via poll).
    pub fn traceroute(&mut self, src: DeviceId, dst_ip: &str) -> bool {
        let Some(dst) = parse_ip(dst_ip) else {
            return false;
        };
        let tick = self.tick;
        let Some(d) = self.devices.get_mut(&src) else {
            return false;
        };
        if !d.powered {
            return false;
        }
        d.probe_trace(dst, tick, &mut self.events);
        true
    }

    /// Single ICMP echo with an explicit TTL — the silent primitive used by
    /// the JS-side traceroute loop. Emits no probe events.
    pub fn ping_ttl(&mut self, src: DeviceId, dst_ip: &str, ttl: u8) -> bool {
        if ttl == 0 {
            return false;
        }
        let Some(dst) = parse_ip(dst_ip) else {
            return false;
        };
        let Some(d) = self.devices.get_mut(&src) else {
            return false;
        };
        if !d.powered {
            return false;
        }
        let frame = Frame::ip([0, 0, 0, 0], dst, ttl, Payload::IcmpEchoRequest);
        d.emit_ip(frame, self.tick);
        true
    }

    pub fn dhcp_client(&mut self, dev: DeviceId, port: usize) -> bool {
        let tick = self.tick;
        let Some(d) = self.devices.get_mut(&dev) else {
            return false;
        };
        crate::services::start_dhcp(d, port, tick, &mut self.events)
    }

    pub fn dns_query(&mut self, dev: DeviceId, name: &str) -> bool {
        let tick = self.tick;
        let Some(d) = self.devices.get_mut(&dev) else {
            return false;
        };
        crate::services::start_dns(d, name, tick, &mut self.events)
    }

    pub fn probe_tcp(&mut self, dev: DeviceId, dst: &str, port: u16) -> bool {
        let Some(ip) = parse_ip(dst) else {
            return false;
        };
        let tick = self.tick;
        let Some(d) = self.devices.get_mut(&dev) else {
            return false;
        };
        crate::services::start_tcp(d, ip, port, false, tick, &mut self.events)
    }

    pub fn http_get(&mut self, dev: DeviceId, dst: &str, port: u16) -> bool {
        let Some(ip) = parse_ip(dst) else {
            return false;
        };
        let tick = self.tick;
        let Some(d) = self.devices.get_mut(&dev) else {
            return false;
        };
        crate::services::start_tcp(d, ip, port, true, tick, &mut self.events)
    }

    pub fn probe_udp(&mut self, dev: DeviceId, dst: &str, port: u16, payload: &str) -> bool {
        let Some(ip) = parse_ip(dst) else {
            return false;
        };
        let tick = self.tick;
        let Some(d) = self.devices.get_mut(&dev) else {
            return false;
        };
        crate::services::start_udp(d, ip, port, payload.as_bytes(), tick, &mut self.events)
    }
}

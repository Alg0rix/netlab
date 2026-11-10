//! User-triggered actions: ping and L4/L7 service probes.

use crate::frame::{Frame, Payload};
use crate::parse_ip;
use crate::DeviceId;

use super::Topology;

impl Topology {
    /// Start an ICMP echo from `src` (a host or router) to `dst_ip`.
    pub fn ping(&mut self, src: DeviceId, dst_ip: &str) -> bool {
        self.ping_ttl(src, dst_ip, 64)
    }

    /// ICMP echo with an explicit TTL. Traceroute sends these one hop at a time.
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
        d.emit_ip(frame, self.tick, &mut self.events);
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

use std::collections::HashSet;
use std::net::{IpAddr, Ipv4Addr, UdpSocket};
use std::process::Command;

use octomonitor_core::AdvertisedAddress;

fn is_wireguard_iface(name: &str) -> bool {
    name.starts_with("wg") || name.starts_with("utun")
}

fn classify_address(name: &str, ip: IpAddr) -> Option<(&'static str, &'static str)> {
    if ip.is_loopback() {
        return None;
    }
    match ip {
        IpAddr::V4(v4) if is_tailscale_ipv4(v4) => Some(("tailscale", "Tailscale")),
        _ if is_wireguard_iface(name) => Some(("private", "Private network")),
        IpAddr::V4(v4) if v4.is_private() => Some(("lan", "LAN")),
        _ => None,
    }
}

fn is_tailscale_ipv4(ip: Ipv4Addr) -> bool {
    let [a, b, ..] = ip.octets();
    a == 100 && (64..=127).contains(&b)
}

pub fn detect_advertised_addresses(port: u16) -> Vec<AdvertisedAddress> {
    let mut seen = HashSet::new();
    let mut addresses = Vec::new();

    if let Ok(output) = Command::new("ifconfig").output() {
        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let mut current_iface = String::new();

            for line in stdout.lines() {
                if !line.starts_with('\t') && !line.starts_with(' ') {
                    current_iface = line
                        .split(':')
                        .next()
                        .unwrap_or_default()
                        .trim()
                        .to_string();
                    continue;
                }

                let ip = line
                    .trim()
                    .strip_prefix("inet ")
                    .and_then(|rest| rest.split_whitespace().next())
                    .and_then(|text| text.parse::<Ipv4Addr>().ok());
                if let Some(ip) = ip {
                    push_address(
                        &mut seen,
                        &mut addresses,
                        port,
                        &current_iface,
                        IpAddr::V4(ip),
                    );
                }
            }
        }
    }

    if addresses.is_empty() {
        if let Some(ip) = detect_primary_ip() {
            push_address(&mut seen, &mut addresses, port, "primary", IpAddr::V4(ip));
        }
    }

    addresses.sort_by_key(|address| match address.kind.as_str() {
        "tailscale" => 0,
        "private" => 1,
        "lan" => 2,
        _ => 3,
    });

    addresses
}

fn detect_primary_ip() -> Option<Ipv4Addr> {
    let socket = UdpSocket::bind("0.0.0.0:0").ok()?;
    socket.connect("8.8.8.8:80").ok()?;
    let addr = socket.local_addr().ok()?;
    match addr.ip() {
        IpAddr::V4(ip) => Some(ip),
        IpAddr::V6(_) => None,
    }
}

fn push_address(
    seen: &mut HashSet<String>,
    addresses: &mut Vec<AdvertisedAddress>,
    port: u16,
    iface_name: &str,
    ip: IpAddr,
) {
    let Some((kind, label)) = classify_address(iface_name, ip) else {
        return;
    };
    let host = ip.to_string();
    let dedupe_key = format!("{kind}:{host}");
    if !seen.insert(dedupe_key) {
        return;
    }
    addresses.push(AdvertisedAddress {
        kind: kind.to_string(),
        label: label.to_string(),
        url: format!("http://{host}:{port}"),
        host,
        port,
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tailscale_range_is_classified() {
        let ip = Ipv4Addr::new(100, 100, 10, 1);
        assert!(is_tailscale_ipv4(ip));
    }

    #[test]
    fn private_lan_addresses_are_classified() {
        let result = classify_address("en0", IpAddr::V4(Ipv4Addr::new(192, 168, 1, 24)));
        assert_eq!(result, Some(("lan", "LAN")));
    }
}

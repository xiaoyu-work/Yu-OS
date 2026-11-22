//! A public IP alone is not proof of Internet routing: it can belong to
//! this machine, an on-link network or an owner-managed subnet.

use std::io;
use std::net::{IpAddr, SocketAddr};
use std::os::fd::AsRawFd;
use std::time::Duration;

use socket2::{Domain, Protocol, Socket, Type};

const HEADER: usize = 16; // nlmsghdr
const ROUTE: usize = 12; // rtmsg
const FIB_MATCH: u32 = 0x2000; // RTM_F_FIB_MATCH
const RTA_IP_PROTO: u16 = 27;
const RTA_DPORT: u16 = 29;

pub(super) fn pin(addresses: &[SocketAddr], stopping: impl Fn() -> bool) -> io::Result<SocketAddr> {
    if addresses
        .iter()
        .any(|address| !internet_address(address.ip()))
    {
        return Err(denied());
    }
    let socket = Socket::new(
        Domain::from(libc::AF_NETLINK),
        Type::RAW,
        Some(Protocol::from(libc::NETLINK_ROUTE)),
    )?;
    socket.set_read_timeout(Some(Duration::from_millis(100)))?;
    let mut pinned = None;
    for address in addresses {
        if stopping() {
            return Err(io::Error::other(
                "public Internet egress retired during route lookup",
            ));
        }
        // No route (e.g. no configured IPv6 on a dual-stack answer) can
        // be ignored; a route to any local/specific answer refuses the name.
        if via_default_gateway(&socket, *address)? {
            pinned.get_or_insert(*address);
        }
    }
    pinned.ok_or_else(denied)
}

fn internet_address(ip: IpAddr) -> bool {
    if !super::is_globally_routable(ip) {
        return false;
    }
    match ip {
        IpAddr::V4(ip) => {
            let bytes = ip.octets();
            // Azure WireServer and the retired 6to4 relay anycast range.
            bytes != [168, 63, 129, 16] && bytes[..3] != [192, 88, 99]
        }
        IpAddr::V6(ip) => {
            let [first, second, ..] = ip.segments();
            (0x2000..0x4000).contains(&first)
                && !(first == 0x2001 && (second < 0x0200 || second == 0x0db8))
                && first != 0x2002
                && !(first == 0x3fff && second < 0x1000)
        }
    }
}

fn attr(message: &mut Vec<u8>, kind: u16, value: &[u8]) {
    let length = 4 + value.len();
    message.extend((length as u16).to_ne_bytes());
    message.extend(kind.to_ne_bytes());
    message.extend(value);
    message.resize(message.len().next_multiple_of(4), 0);
}

fn via_default_gateway(socket: &Socket, address: SocketAddr) -> io::Result<bool> {
    let mut request = vec![0; HEADER + ROUTE];
    request[4..6].copy_from_slice(&libc::RTM_GETROUTE.to_ne_bytes());
    request[6..8].copy_from_slice(&(libc::NLM_F_REQUEST as u16).to_ne_bytes());
    request[8..12].copy_from_slice(&1u32.to_ne_bytes());
    let (family, octets) = match address.ip() {
        IpAddr::V4(ip) => (libc::AF_INET as u8, ip.octets().to_vec()),
        IpAddr::V6(ip) => (libc::AF_INET6 as u8, ip.octets().to_vec()),
    };
    request[HEADER] = family;
    request[HEADER + 1] = (octets.len() * 8) as u8;
    request[HEADER + 8..HEADER + ROUTE].copy_from_slice(&FIB_MATCH.to_ne_bytes());
    attr(&mut request, libc::RTA_DST, &octets);
    attr(&mut request, RTA_IP_PROTO, &[libc::IPPROTO_TCP as u8]);
    attr(&mut request, RTA_DPORT, &address.port().to_be_bytes());
    let length = request.len() as u32;
    request[..4].copy_from_slice(&length.to_ne_bytes());

    let mut kernel: libc::sockaddr_nl = unsafe { std::mem::zeroed() };
    kernel.nl_family = libc::AF_NETLINK as libc::sa_family_t;
    let size = std::mem::size_of::<libc::sockaddr_nl>() as libc::socklen_t;
    let sent = unsafe {
        libc::sendto(
            socket.as_raw_fd(),
            request.as_ptr().cast(),
            request.len(),
            0,
            (&kernel as *const libc::sockaddr_nl).cast(),
            size,
        )
    };
    if sent < 0 {
        return Err(io::Error::last_os_error());
    }
    if sent as usize != request.len() {
        return Err(invalid_reply());
    }
    let mut reply = [0u8; 4096];
    let mut sender: libc::sockaddr_nl = unsafe { std::mem::zeroed() };
    let mut sender_size = size;
    let received = unsafe {
        libc::recvfrom(
            socket.as_raw_fd(),
            reply.as_mut_ptr().cast(),
            reply.len(),
            libc::MSG_TRUNC,
            (&mut sender as *mut libc::sockaddr_nl).cast(),
            &mut sender_size,
        )
    };
    if received < 0 {
        return Err(io::Error::last_os_error());
    }
    if sender_size != size || sender.nl_family != kernel.nl_family || sender.nl_pid != 0 {
        return Err(invalid_reply());
    }
    check_route(
        reply.get(..received as usize).ok_or_else(invalid_reply)?,
        family,
        &octets,
    )
}

fn check_route(reply: &[u8], family: u8, destination: &[u8]) -> io::Result<bool> {
    let header = reply.get(..HEADER).ok_or_else(invalid_reply)?;
    let length = u32::from_ne_bytes(header[..4].try_into().unwrap()) as usize;
    if length != reply.len() || header[8..12] != 1u32.to_ne_bytes() {
        return Err(invalid_reply());
    }
    let kind = u16::from_ne_bytes([header[4], header[5]]);
    let payload = &reply[HEADER..];
    if kind == libc::NLMSG_ERROR as u16 {
        let error: [u8; 4] = payload
            .get(..4)
            .ok_or_else(invalid_reply)?
            .try_into()
            .unwrap();
        return match i32::from_ne_bytes(error) {
            error if error == -libc::ENETUNREACH || error == -libc::EHOSTUNREACH => Ok(false),
            _ => Err(denied()),
        };
    }
    let route = payload.get(..ROUTE).ok_or_else(invalid_reply)?;
    if kind != libc::RTM_NEWROUTE || route[0] != family {
        return Err(invalid_reply());
    }
    // Local/broadcast destinations and more-specific routes (including
    // on-link or gateway-routed LAN prefixes) are not Internet routes.
    if route[1] != 0 || route[7] != libc::RTN_UNICAST || route[6] != libc::RT_SCOPE_UNIVERSE {
        return Err(denied());
    }
    let mut attrs = &payload[ROUTE..];
    while !attrs.is_empty() {
        let [lo, hi, kind_lo, kind_hi, ..] = attrs else {
            return Err(invalid_reply());
        };
        let length = u16::from_ne_bytes([*lo, *hi]) as usize;
        if length < 4 || length > attrs.len() {
            return Err(invalid_reply());
        }
        if u16::from_ne_bytes([*kind_lo, *kind_hi]) == libc::RTA_GATEWAY {
            let gateway = &attrs[4..length];
            // A point-to-point or on-link gateway may not have its own
            // prefix route. It is still a local target, not an Internet peer.
            return if gateway.len() == destination.len() && gateway != destination {
                Ok(true)
            } else {
                Err(denied())
            };
        }
        attrs = attrs
            .get(length.next_multiple_of(4)..)
            .ok_or_else(invalid_reply)?;
    }
    Err(denied())
}

fn denied() -> io::Error {
    io::Error::new(
        io::ErrorKind::PermissionDenied,
        "destination is outside the public Internet route",
    )
}

fn invalid_reply() -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidData,
        "invalid Linux route lookup reply",
    )
}

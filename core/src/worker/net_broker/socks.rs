//! SOCKS5 CONNECT over the already authenticated Unix stream.

use std::io::{self, Read, Write};
use std::net::{Ipv4Addr, Ipv6Addr};

use super::Endpoint;

// The caller has already consumed the SOCKS version byte.
pub(super) fn connect_request(stream: &mut (impl Read + Write)) -> io::Result<Endpoint> {
    let mut methods = [0; 255];
    let count = byte(stream)? as usize;
    stream.read_exact(&mut methods[..count])?;
    let no_auth = methods[..count].contains(&0);
    stream.write_all(&[5, if no_auth { 0 } else { 0xff }])?;
    if !no_auth {
        return Err(io::Error::other(
            "SOCKS peer authentication is via the Unix socket",
        ));
    }
    match request(stream) {
        Ok(endpoint) => Ok(endpoint),
        Err(code) => {
            reply(stream, code)?;
            Err(io::Error::other("invalid or unsupported SOCKS request"))
        }
    }
}

fn request(stream: &mut impl Read) -> Result<Endpoint, u8> {
    let mut prefix = [0; 4];
    stream.read_exact(&mut prefix).map_err(|_| 1u8)?;
    let [5, command, 0, address_type] = prefix else {
        return Err(1);
    };
    if command != 1 {
        return Err(7); // No BIND or UDP ASSOCIATE.
    }
    let host = match address_type {
        1 => {
            let mut address = [0; 4];
            stream.read_exact(&mut address).map_err(|_| 1u8)?;
            Ipv4Addr::from(address).to_string()
        }
        3 => {
            let length = byte(stream).map_err(|_| 1u8)? as usize;
            let mut name = [0; 255];
            stream.read_exact(&mut name[..length]).map_err(|_| 1u8)?;
            let name = std::str::from_utf8(&name[..length]).map_err(|_| 8u8)?;
            name.strip_suffix('.').unwrap_or(name).to_string()
        }
        4 => {
            let mut address = [0; 16];
            stream.read_exact(&mut address).map_err(|_| 1u8)?;
            Ipv6Addr::from(address).to_string()
        }
        _ => return Err(8),
    };
    let mut port = [0; 2];
    stream.read_exact(&mut port).map_err(|_| 1u8)?;
    let endpoint = Endpoint::new(host, u16::from_be_bytes(port));
    super::super::policy::validate_endpoint(&endpoint).map_err(|_| 2u8)?;
    Ok(endpoint)
}

fn byte(stream: &mut impl Read) -> io::Result<u8> {
    let mut value = [0];
    stream.read_exact(&mut value)?;
    Ok(value[0])
}

pub(super) fn reply(stream: &mut impl Write, status: u8) -> io::Result<()> {
    // CONNECT does not expose a broker-side listener or bind address.
    stream.write_all(&[5, status, 0, 1, 0, 0, 0, 0, 0, 0])
}

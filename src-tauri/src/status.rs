use std::{
    net::{IpAddr, SocketAddr},
    time::Duration,
};

use hickory_resolver::{proto::rr::RData, TokioResolver};
use serde::{Deserialize, Serialize};
use tokio::{
    io::{AsyncRead, AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
    time::timeout,
};

const DEFAULT_PORT: u16 = 25565;
const STATUS_PROTOCOL: u8 = 47;
const MAX_PACKET_SIZE: usize = 128 * 1024;

#[derive(Debug, Deserialize, PartialEq, Serialize)]
pub struct Players {
    online: u32,
    max: u32,
}

pub async fn query(address: &str) -> Option<Players> {
    let (host, port) = split_address(address)?;
    timeout(Duration::from_secs(5), async {
        let resolved = timeout(Duration::from_secs(2), resolve_srv(host))
            .await
            .ok()
            .flatten();
        let (host, port) = resolved
            .as_ref()
            .map_or((host, port), |(host, port)| (host.as_str(), *port));
        let mut stream = TcpStream::connect((host, port)).await.ok()?;
        stream.write_all(&request_packet(host, port)).await.ok()?;
        read_players(&mut stream).await
    })
    .await
    .ok()
    .flatten()
}

fn split_address(address: &str) -> Option<(&str, u16)> {
    if address.parse::<IpAddr>().is_ok() {
        return Some((address, DEFAULT_PORT));
    }
    if let Some(bracketed) = address.strip_prefix('[') {
        let (host, suffix) = bracketed.split_once(']')?;
        let port = if suffix.is_empty() {
            host.parse::<std::net::Ipv6Addr>().ok()?;
            DEFAULT_PORT
        } else {
            address.parse::<SocketAddr>().ok()?.port()
        };
        return (port != 0).then_some((host, port));
    }
    let (host, port) = match address.rsplit_once(':') {
        Some((host, port)) => (host, port.parse().ok()?),
        None => (address, DEFAULT_PORT),
    };
    (!host.is_empty() && host.len() <= 255 && !host.contains(':') && port != 0)
        .then_some((host, port))
}

async fn resolve_srv(host: &str) -> Option<(String, u16)> {
    if host.parse::<IpAddr>().is_ok() {
        return None;
    }
    let resolver = TokioResolver::builder_tokio().ok()?.build().ok()?;
    let records = resolver
        .srv_lookup(format!("_minecraft._tcp.{}.", host.trim_end_matches('.')))
        .await
        .ok()?;
    records
        .answers()
        .iter()
        .find_map(|record| match &record.data {
            RData::SRV(srv) if !srv.target.is_root() && srv.port != 0 => Some((
                srv.target.to_ascii().trim_end_matches('.').to_owned(),
                srv.port,
            )),
            _ => None,
        })
}

fn request_packet(host: &str, port: u16) -> Vec<u8> {
    // Keep the handshake protocol and SRV target consistent with the Electron launcher.
    let mut handshake = vec![0, STATUS_PROTOCOL];
    write_varint(&mut handshake, host.len() as u32);
    handshake.extend(host.bytes());
    handshake.extend(port.to_be_bytes());
    handshake.push(1);
    let mut request = Vec::new();
    write_varint(&mut request, handshake.len() as u32);
    request.extend(handshake);
    request.extend([1, 0]);
    request
}

async fn read_players(reader: &mut (impl AsyncRead + Unpin)) -> Option<Players> {
    #[derive(Deserialize)]
    struct Status {
        players: Players,
    }

    let length = read_varint(reader).await? as usize;
    if length > MAX_PACKET_SIZE {
        return None;
    }
    let mut packet = vec![0; length];
    reader.read_exact(&mut packet).await.ok()?;
    let mut payload = packet.as_slice();
    if read_varint(&mut payload).await? != 0
        || read_varint(&mut payload).await? as usize != payload.len()
    {
        return None;
    }
    serde_json::from_slice::<Status>(payload)
        .ok()
        .map(|status| status.players)
}

fn write_varint(bytes: &mut Vec<u8>, mut value: u32) {
    while value >= 0x80 {
        bytes.push((value as u8 & 0x7f) | 0x80);
        value >>= 7;
    }
    bytes.push(value as u8);
}

async fn read_varint(reader: &mut (impl AsyncRead + Unpin)) -> Option<u32> {
    let mut value = 0;
    for shift in [0, 7, 14, 21, 28] {
        let byte = reader.read_u8().await.ok()?;
        if shift == 28 && byte > 0x0f {
            return None;
        }
        value |= u32::from(byte & 0x7f) << shift;
        if byte & 0x80 == 0 {
            return Some(value);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn queries_a_server_and_reads_player_counts() {
        tauri::async_runtime::block_on(async {
            let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            let address = listener.local_addr().unwrap();
            let peer = tokio::spawn(async move {
                let (mut stream, _) = listener.accept().await.unwrap();
                let mut request = [0; 18];
                stream.read_exact(&mut request).await.unwrap();
                let [high, low] = address.port().to_be_bytes();
                assert_eq!(
                    request,
                    [
                        15, 0, 47, 9, b'1', b'2', b'7', b'.', b'0', b'.', b'0', b'.', b'1', high,
                        low, 1, 1, 0
                    ]
                );
                let json = br#"{"players":{"online":3,"max":20}}"#;
                stream
                    .write_all(&[json.len() as u8 + 2, 0, json.len() as u8])
                    .await
                    .unwrap();
                stream.write_all(json).await.unwrap();
            });
            assert_eq!(
                query(&address.to_string()).await,
                Some(Players { online: 3, max: 20 })
            );
            peer.await.unwrap();
        });
    }

    #[test]
    fn rejects_oversized_truncated_and_invalid_packets() {
        tauri::async_runtime::block_on(async {
            for mut packet in [
                &[0xff, 0xff, 0xff, 0xff, 0x10][..],
                &[0x81, 0x80, 0x08],
                &[3, 0, 2, b'{'],
                &[3, 1, 1, b'{'],
                &[3, 0, 2, b'{', b'}'],
            ] {
                assert_eq!(read_players(&mut packet).await, None);
            }
        });
    }
}

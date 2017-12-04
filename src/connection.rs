use packet::{self, Packet, PacketError};

use std::net::{SocketAddr, UdpSocket};

const TIMEOUT_TIME: f64 = 5.0;
const KEEPALIVE_TIME: f64 = 1.0;

pub enum UpdateResult {
    SentKeepAlive,
    Expired,
    Nil
}

#[derive(Debug)]
pub struct Connection {
    addr: SocketAddr,
    sequence: u32,
    client_id: u8,
    protocol_id: u32,

    last_sent: f64,
    last_received: f64
}

impl Connection {
    pub fn new(addr: &SocketAddr, client_id: u8, protocol_id: u32, time: f64) -> Connection {
        Connection {
            addr: addr.clone(), sequence: 0, client_id, protocol_id,
            last_sent: time, last_received: time
        }
    }

    pub fn send(&mut self, packet: &Packet, socket: &UdpSocket, time: f64) -> Result<usize, PacketError> {
        let mut buf = [0; 256];
        let len = packet::encode(&mut buf, self.protocol_id, packet)?;
        socket.send_to(&buf[..len], &self.addr)?;
        self.sequence += 1;

        self.last_sent = time;
        Ok(len)
    }

    pub fn recv(&mut self, packet: &[u8], time: f64) -> Result<Packet, PacketError> {
        let (_, packet) = packet::decode(packet, self.protocol_id)?;

        self.last_received = time;
        Ok(packet)
    }

    pub fn get_id(&self) -> u8 {
        self.client_id
    }

    pub fn get_address(&self) -> SocketAddr {
        self.addr
    }

    pub fn tick(&mut self, socket: &UdpSocket, time: f64) -> Result<UpdateResult, PacketError> {
        if self.last_sent + KEEPALIVE_TIME < time {
            self.send(&Packet::KeepAlive, socket, time)?;
            return Ok(UpdateResult::SentKeepAlive)
        }

        if self.last_received + TIMEOUT_TIME < time {
            return Ok(UpdateResult::Expired)
        }

        Ok(UpdateResult::Nil)
    }
}

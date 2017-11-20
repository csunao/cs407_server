use packet::{self, Packet, PacketError};

use std::net::{UdpSocket, SocketAddr};

#[derive(Debug)]
pub struct Connection {
    addr: SocketAddr,
    sequence: u32,
    client_id: u8,
    protocol_id: u32
}

impl Connection {
    pub fn new(addr: &SocketAddr, client_id: u8, protocol_id: u32) -> Connection {
        Connection {
            addr: addr.clone(), sequence: 0, client_id, protocol_id
        }
    }

    pub fn send(&mut self, packet: &Packet, socket: &UdpSocket) -> Result<usize, PacketError> {
        let mut buf = [0; 256];
        let len = packet::encode(&mut buf, self.protocol_id, packet)?;
        println!("sending {:?}", &buf[..len]);
        socket.send_to(&buf[..len], &self.addr)?;
        self.sequence += 1;

        Ok(len)
    }

    pub fn recv(&self, packet: &[u8]) -> Result<Packet, PacketError> {
        let (sequence, packet) = packet::decode(packet, self.protocol_id)?;

        Ok(packet)
    }

    pub fn get_id(&self) -> u8 {
        self.client_id
    }
}

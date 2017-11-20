use std::io;

pub const PACKET_CONNECT:    u8 = 0;
pub const PACKET_CONFIRM:    u8 = 1;
pub const PACKET_DENY:       u8 = 2;
pub const PACKET_KEEPALIVE:  u8 = 3;
pub const PACKET_PAYLOAD:    u8 = 4;
pub const PACKET_DISCONNECT: u8 = 5;

#[derive(Debug, PartialEq, Eq)]
pub enum Packet {
    Connect,
    Confirm(ConfirmPacket),
    Deny,
    KeepAlive,
    Payload(PayloadPacket),
    Disconnect,
}

#[derive(Debug)]
pub enum PacketError {
    InvalidPacket,
    GenericIO(io::Error)
}

impl From<io::Error> for PacketError {
    fn from(err: io::Error) -> PacketError {
        PacketError::GenericIO(err)
    }
}

pub fn decode(data: &[u8], protocol_id: u32) -> Result<(u64, Packet), PacketError> {
    let mut source = &mut io::Cursor::new(data);

    let packet_protocol_id = read_u32(source)?;
    let packet_type = read_u8(source)?;

    if protocol_id != packet_protocol_id {
        return Err(PacketError::InvalidPacket);
    }

    match packet_type {
        PACKET_CONNECT =>
            Ok(Packet::Connect),
        PACKET_CONFIRM =>
            Ok(Packet::Confirm(ConfirmPacket::from(&mut source).unwrap())),
        PACKET_DENY =>
            Ok(Packet::Deny),
        PACKET_KEEPALIVE =>
            Ok(Packet::KeepAlive),
        PACKET_PAYLOAD =>
            Ok(Packet::Payload(PayloadPacket::from(&mut source).unwrap())),
        PACKET_DISCONNECT =>
            Ok(Packet::Disconnect),
        _ => Err(PacketError::InvalidPacket)
    }.map(|p| (0, p))
}

pub fn encode(out: &mut [u8], protocol_id: u32, packet: &Packet) -> Result<usize, PacketError> {
    let mut writer = io::Cursor::new(&mut out[..]);

    write_u32(&mut writer, protocol_id)?;
    write_u8(&mut writer, packet.get_type_id())?;

    match *packet {
        Packet::Payload(ref payload) => {
            payload.write(&mut writer)?;
        },
        Packet::Confirm(ref confirm) => {
            confirm.write(&mut writer)?;
        },
        _ => {}
    }

    Ok(writer.position() as usize)
}

impl Packet {
    pub fn get_type_id(&self) -> u8 {
        match *self {
            Packet::Connect => PACKET_CONNECT,
            Packet::Confirm(_) => PACKET_CONFIRM,
            Packet::Deny => PACKET_DENY,
            Packet::KeepAlive => PACKET_KEEPALIVE,
            Packet::Payload(_) => PACKET_PAYLOAD,
            Packet::Disconnect => PACKET_DISCONNECT
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct PayloadPacket {
    pub bytes: Vec<u8>
}

impl PayloadPacket {
    fn from<R>(reader: &mut R) -> io::Result<PayloadPacket> where R: io::Read {
        let amt = read_u8(reader)? as usize;
        let mut bytes: [u8; 256] = [0; 256];
        let num_bytes = reader.read(&mut bytes[..amt])?;
        assert!(amt == num_bytes);
        let mut bytes = bytes.to_vec();
        bytes.truncate(amt);
        Ok(PayloadPacket { bytes: bytes.to_vec() })
    }

    fn write<W>(&self, writer: &mut W) -> io::Result<usize> where W: io::Write {
        write_u8(writer, self.bytes.len() as u8)?;
        writer.write(&self.bytes[..])
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct ConfirmPacket {
    pub client_id: u8
}

impl ConfirmPacket {
    fn from<R>(reader: &mut R) -> io::Result<ConfirmPacket> where R: io::Read {
        let client_id = read_u8(reader)?;
        Ok(ConfirmPacket { client_id })
    }

    fn write<W>(&self, writer: &mut W) -> io::Result<usize> where W: io::Write {
        write_u8(writer, self.client_id)
    }
}

// output: num written into writer
// process: convert num into bytes & write (via transmute?)
fn write_u8<W>(writer: &mut W, num: u8) -> io::Result<usize> where W: io::Write {
    writer.write(&[num])
}

fn _write_u16<W>(writer: &mut W, num: u16) -> io::Result<usize> where W: io::Write {
    writer.write(&[
        ((num >> 8) & 0xFF) as u8,
        (num & 0xFF) as u8])
}

fn write_u32<W>(writer: &mut W, num: u32) -> io::Result<usize> where W: io::Write {
    writer.write(&[
        ((num >> 24) & 0xFF) as u8,
        ((num >> 16) & 0xFF) as u8,
        ((num >> 8) & 0xFF) as u8,
        (num & 0xFF) as u8])
}

fn _write_u64<W>(writer: &mut W, num: u64) -> io::Result<usize> where W: io::Write {
    writer.write(&[
        ((num >> 56) & 0xFF) as u8,
        ((num >> 48) & 0xFF) as u8,
        ((num >> 40) & 0xFF) as u8,
        ((num >> 32) & 0xFF) as u8,
        ((num >> 24) & 0xFF) as u8,
        ((num >> 16) & 0xFF) as u8,
        ((num >> 8) & 0xFF) as u8,
        (num & 0xFF) as u8])
}

macro_rules! bytes_to_num {
    ($bytes: ident, $type: ty) => ({
        let mut result: $type = 0;
        for byte in &$bytes {
            result <<= 8;
            result += *byte as $type;
        }
        Ok(result)
    });
}

fn read_u8<R>(reader: &mut R) -> io::Result<u8> where R: io::Read {
    let mut byte = [0; 1];
    let _ = reader.read_exact(&mut byte)?;
    Ok(byte[0])
}

fn _read_u16<R>(reader: &mut R) -> io::Result<u16> where R: io::Read {
    let mut bytes = [0; 2];
    let _ = reader.read_exact(&mut bytes)?;
    bytes_to_num!(bytes, u16)
}

fn read_u32<R>(reader: &mut R) -> io::Result<u32> where R: io::Read {
    let mut bytes = [0; 4];
    let _ = reader.read_exact(&mut bytes)?;
    bytes_to_num!(bytes, u32)
}

fn _read_u64<R>(reader: &mut R) -> io::Result<u64> where R: io::Read {
    let mut bytes = [0; 8];
    let _ = reader.read_exact(&mut bytes)?;
    bytes_to_num!(bytes, u64)
}

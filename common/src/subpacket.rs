// garlemald-server — Rust port of a FINAL FANTASY XIV v1.23b server emulator (lobby/world/map)
// Copyright (C) 2026  Samuel Stegall
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU Affero General Public License as published
// by the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU Affero General Public License for more details.
//
// You should have received a copy of the GNU Affero General Public License
// along with this program.  If not, see <https://www.gnu.org/licenses/>.
//
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::io::Cursor;

use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};

use crate::error::PacketError;
use crate::utils;

pub const SUBPACKET_SIZE: usize = 0x10;
pub const GAMEMESSAGE_SIZE: usize = 0x10;

pub const SUBPACKET_TYPE_GAMEMESSAGE: u16 = 0x03;

#[derive(Debug, Clone, Copy, Default)]
pub struct SubPacketHeader {
    pub subpacket_size: u16,
    pub r#type: u16,
    pub source_id: u32,
    pub target_id: u32,
    pub unknown1: u32,
}

impl SubPacketHeader {
    pub fn read(buf: &[u8]) -> Result<Self, PacketError> {
        if buf.len() < SUBPACKET_SIZE {
            return Err(PacketError::TooSmall {
                needed: SUBPACKET_SIZE,
                have: buf.len(),
            });
        }
        let mut c = Cursor::new(buf);
        Ok(Self {
            subpacket_size: c.read_u16::<LittleEndian>()?,
            r#type: c.read_u16::<LittleEndian>()?,
            source_id: c.read_u32::<LittleEndian>()?,
            target_id: c.read_u32::<LittleEndian>()?,
            unknown1: c.read_u32::<LittleEndian>()?,
        })
    }

    pub fn write(&self, out: &mut [u8; SUBPACKET_SIZE]) {
        let mut c = Cursor::new(&mut out[..]);
        c.write_u16::<LittleEndian>(self.subpacket_size).unwrap();
        c.write_u16::<LittleEndian>(self.r#type).unwrap();
        c.write_u32::<LittleEndian>(self.source_id).unwrap();
        c.write_u32::<LittleEndian>(self.target_id).unwrap();
        c.write_u32::<LittleEndian>(self.unknown1).unwrap();
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct GameMessageHeader {
    pub unknown4: u16,
    pub opcode: u16,
    pub unknown5: u32,
    pub timestamp: u32,
    pub unknown6: u32,
}

impl GameMessageHeader {
    pub fn read(buf: &[u8]) -> Result<Self, PacketError> {
        if buf.len() < GAMEMESSAGE_SIZE {
            return Err(PacketError::TooSmall {
                needed: GAMEMESSAGE_SIZE,
                have: buf.len(),
            });
        }
        let mut c = Cursor::new(buf);
        Ok(Self {
            unknown4: c.read_u16::<LittleEndian>()?,
            opcode: c.read_u16::<LittleEndian>()?,
            unknown5: c.read_u32::<LittleEndian>()?,
            timestamp: c.read_u32::<LittleEndian>()?,
            unknown6: c.read_u32::<LittleEndian>()?,
        })
    }

    pub fn write(&self, out: &mut [u8; GAMEMESSAGE_SIZE]) {
        let mut c = Cursor::new(&mut out[..]);
        c.write_u16::<LittleEndian>(self.unknown4).unwrap();
        c.write_u16::<LittleEndian>(self.opcode).unwrap();
        c.write_u32::<LittleEndian>(self.unknown5).unwrap();
        c.write_u32::<LittleEndian>(self.timestamp).unwrap();
        c.write_u32::<LittleEndian>(self.unknown6).unwrap();
    }
}

#[derive(Debug, Clone)]
pub struct SubPacket {
    pub header: SubPacketHeader,
    pub game_message: GameMessageHeader,
    pub data: Vec<u8>,
}

impl SubPacket {
    /// Mirrors the C# `SubPacket(bool isGameMessage, ushort opcode, uint sourceId, byte[] data)`.
    /// When `is_game_message` is true, the 16-byte game-message header is
    /// prefixed and the subpacket `type` is forced to 0x03 (GameMessage).
    pub fn new_with_flag(
        is_game_message: bool,
        opcode: u16,
        source_id: u32,
        data: Vec<u8>,
    ) -> Self {
        let mut header = SubPacketHeader::default();
        let mut game_message = GameMessageHeader::default();

        if is_game_message {
            game_message.opcode = opcode;
            game_message.timestamp = utils::unix_timestamp();
            game_message.unknown4 = 0x14;
        }

        header.source_id = source_id;
        header.r#type = if is_game_message {
            SUBPACKET_TYPE_GAMEMESSAGE
        } else {
            opcode
        };
        header.subpacket_size = (SUBPACKET_SIZE + data.len()) as u16;
        if is_game_message {
            header.subpacket_size += GAMEMESSAGE_SIZE as u16;
        }

        SubPacket {
            header,
            game_message,
            data,
        }
    }

    /// Convenience constructor: game-message subpacket.
    pub fn new(opcode: u16, source_id: u32, data: Vec<u8>) -> Self {
        Self::new_with_flag(true, opcode, source_id, data)
    }

    /// Re-target an existing subpacket (used when relaying to a different actor).
    pub fn with_target(other: &SubPacket, new_target: u32) -> Self {
        let mut header = other.header;
        header.target_id = new_target;
        SubPacket {
            header,
            game_message: other.game_message,
            data: other.data.clone(),
        }
    }

    pub fn set_target_id(&mut self, target: u32) {
        self.header.target_id = target;
    }

    /// Stamp `target` into the `target_id` field of every subpacket in a
    /// serialized subpacket stream whose `target_id` is still 0. Returns
    /// the number of headers stamped.
    ///
    /// The world-server proxy fans map-server replies out to client
    /// sessions purely by each subpacket's `target_id`, and DROPS
    /// untargeted (`target_id == 0`) subpackets (`world-server/src/
    /// server.rs`). Builders construct subpackets with a zeroed header,
    /// so any send path that forgets `set_target_id` produces packets
    /// that silently never reach the client — this helper lets fan-out
    /// helpers (`broadcast_around_actor`, `send_to_self_if_player`)
    /// stamp the recipient's session id at send time instead of relying
    /// on every call site to remember. Subpackets already carrying a
    /// non-zero target are left untouched.
    pub fn stamp_target_id_if_zero(bytes: &mut [u8], target: u32) -> usize {
        let mut stamped = 0usize;
        let mut off = 0usize;
        while off + SUBPACKET_SIZE <= bytes.len() {
            let size = u16::from_le_bytes([bytes[off], bytes[off + 1]]) as usize;
            if size < SUBPACKET_SIZE || off + size > bytes.len() {
                break;
            }
            // Header layout: size u16, type u16, source_id u32, target_id u32.
            let t = off + 8;
            if bytes[t..t + 4] == [0, 0, 0, 0] {
                bytes[t..t + 4].copy_from_slice(&target.to_le_bytes());
                stamped += 1;
            }
            off += size;
        }
        stamped
    }

    pub fn parse(bytes: &[u8], offset: &mut usize) -> Result<SubPacket, PacketError> {
        if bytes.len() < *offset + SUBPACKET_SIZE {
            return Err(PacketError::TooSmall {
                needed: *offset + SUBPACKET_SIZE,
                have: bytes.len(),
            });
        }

        let header = SubPacketHeader::read(&bytes[*offset..*offset + SUBPACKET_SIZE])?;

        // A declared size smaller than the header(s) means the buffer is
        // not a subpacket stream (e.g. a BasePacket frame fed in by
        // mistake — its first u16 is the is_authenticated/is_compressed
        // pair). Without this check the data-slice arithmetic below
        // underflows and panics the caller (the world-server's zone reply
        // pump parses untrusted map-server bytes on a spawned task).
        let min_size = if header.r#type == SUBPACKET_TYPE_GAMEMESSAGE {
            SUBPACKET_SIZE + GAMEMESSAGE_SIZE
        } else {
            SUBPACKET_SIZE
        };
        if (header.subpacket_size as usize) < min_size {
            return Err(PacketError::SizeMismatch {
                declared: header.subpacket_size as usize,
                available: bytes.len() - *offset,
            });
        }

        if bytes.len() < *offset + header.subpacket_size as usize {
            return Err(PacketError::SizeMismatch {
                declared: header.subpacket_size as usize,
                available: bytes.len() - *offset,
            });
        }

        let mut game_message = GameMessageHeader::default();
        let data_start;
        if header.r#type == SUBPACKET_TYPE_GAMEMESSAGE {
            game_message = GameMessageHeader::read(
                &bytes[*offset + SUBPACKET_SIZE..*offset + SUBPACKET_SIZE + GAMEMESSAGE_SIZE],
            )?;
            data_start = *offset + SUBPACKET_SIZE + GAMEMESSAGE_SIZE;
        } else {
            data_start = *offset + SUBPACKET_SIZE;
        }

        let data_end = *offset + header.subpacket_size as usize;
        let data = bytes[data_start..data_end].to_vec();

        *offset += header.subpacket_size as usize;
        Ok(SubPacket {
            header,
            game_message,
            data,
        })
    }

    /// Try to parse a single subpacket from a buffer slice, returning `None`
    /// if there isn't enough data yet. Offset is advanced on success.
    pub fn try_parse(buffer: &[u8], offset: &mut usize, bytes_read: usize) -> Option<SubPacket> {
        if bytes_read <= *offset || buffer.len() < *offset + 2 {
            return None;
        }
        let size = u16::from_le_bytes([buffer[*offset], buffer[*offset + 1]]) as usize;
        if bytes_read < *offset + size || buffer.len() < *offset + size {
            return None;
        }
        SubPacket::parse(buffer, offset).ok()
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let total = self.header.subpacket_size as usize;
        let mut out = vec![0u8; total];

        let mut hdr = [0u8; SUBPACKET_SIZE];
        self.header.write(&mut hdr);
        out[..SUBPACKET_SIZE].copy_from_slice(&hdr);

        let body_start = if self.header.r#type == SUBPACKET_TYPE_GAMEMESSAGE {
            let mut gm = [0u8; GAMEMESSAGE_SIZE];
            self.game_message.write(&mut gm);
            out[SUBPACKET_SIZE..SUBPACKET_SIZE + GAMEMESSAGE_SIZE].copy_from_slice(&gm);
            SUBPACKET_SIZE + GAMEMESSAGE_SIZE
        } else {
            SUBPACKET_SIZE
        };

        out[body_start..body_start + self.data.len()].copy_from_slice(&self.data);
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn game_message_roundtrip() {
        let sub = SubPacket::new(0xBEEF, 0xDEADBEEF, vec![1, 2, 3, 4]);
        let bytes = sub.to_bytes();
        assert_eq!(bytes.len(), sub.header.subpacket_size as usize);

        let mut off = 0;
        let parsed = SubPacket::parse(&bytes, &mut off).unwrap();
        assert_eq!(parsed.header.source_id, 0xDEADBEEF);
        assert_eq!(parsed.header.r#type, SUBPACKET_TYPE_GAMEMESSAGE);
        assert_eq!(parsed.game_message.opcode, 0xBEEF);
        assert_eq!(parsed.data, vec![1, 2, 3, 4]);
        assert_eq!(off, bytes.len());
    }

    #[test]
    fn raw_subpacket_roundtrip() {
        let sub = SubPacket::new_with_flag(false, 0x01, 0x11, vec![9, 8, 7]);
        let bytes = sub.to_bytes();
        let mut off = 0;
        let parsed = SubPacket::parse(&bytes, &mut off).unwrap();
        assert_eq!(parsed.header.r#type, 0x01);
        assert_eq!(parsed.data, vec![9, 8, 7]);
    }

    #[test]
    fn stamp_target_id_if_zero_stamps_untargeted_only() {
        // Two-subpacket stream: first untargeted, second pre-tagged.
        let untagged = SubPacket::new(0x0134, 0x1111, vec![0u8; 8]);
        let mut tagged = SubPacket::new(0x0134, 0x2222, vec![0u8; 8]);
        tagged.set_target_id(0xDEAD_BEEF);
        let mut stream = untagged.to_bytes();
        stream.extend(tagged.to_bytes());

        let stamped = SubPacket::stamp_target_id_if_zero(&mut stream, 42);
        assert_eq!(stamped, 1);

        let mut off = 0usize;
        let first = SubPacket::parse(&stream, &mut off).unwrap();
        let second = SubPacket::parse(&stream, &mut off).unwrap();
        assert_eq!(first.header.target_id, 42);
        assert_eq!(second.header.target_id, 0xDEAD_BEEF);
    }

    #[test]
    fn stamp_target_id_if_zero_ignores_non_subpacket_bytes() {
        // Garbage / truncated buffers must be left untouched.
        let mut short = vec![1u8, 2, 3];
        assert_eq!(SubPacket::stamp_target_id_if_zero(&mut short, 42), 0);
        assert_eq!(short, vec![1, 2, 3]);
    }
}

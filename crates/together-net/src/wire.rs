//! Framing: a little-endian `u32` length, then a `postcard` body.

use std::io::{self, Read, Write};

use serde::de::DeserializeOwned;
use serde::Serialize;

/// The largest message either side accepts: far above a full snapshot, far below harm.
const MAX_FRAME: usize = 8 * 1024 * 1024;

pub(crate) fn encode<T: Serialize>(msg: &T) -> io::Result<Vec<u8>> {
    let body =
        postcard::to_stdvec(msg).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    let mut frame = Vec::with_capacity(4 + body.len());
    frame.extend_from_slice(&(body.len() as u32).to_le_bytes());
    frame.extend_from_slice(&body);
    Ok(frame)
}

pub(crate) fn write_frame(out: &mut impl Write, frame: &[u8]) -> io::Result<()> {
    out.write_all(frame)?;
    out.flush()
}

/// The next message, blocking until it arrives; an error when the stream closes or lies.
pub(crate) fn read<T: DeserializeOwned>(input: &mut impl Read) -> io::Result<T> {
    let mut len = [0u8; 4];
    input.read_exact(&mut len)?;
    let len = u32::from_le_bytes(len) as usize;
    if len > MAX_FRAME {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("a {len}-byte message is too large"),
        ));
    }
    let mut body = vec![0u8; len];
    input.read_exact(&mut body)?;
    postcard::from_bytes(&body).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ClientMsg, HeroPick};

    #[test]
    fn a_message_survives_the_round_trip() {
        let msg = ClientMsg::Pick(HeroPick {
            name: "Jaina".into(),
            class: "mage".into(),
            race: 1,
            sex: 1,
        });
        let frame = encode(&msg).unwrap();
        let back: ClientMsg = read(&mut frame.as_slice()).unwrap();
        assert_eq!(back, msg);
    }

    #[test]
    fn an_oversized_length_is_refused() {
        let frame = (u32::MAX).to_le_bytes();
        assert!(read::<ClientMsg>(&mut frame.as_slice()).is_err());
    }
}

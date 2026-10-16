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
    read_capped(input, MAX_FRAME)
}

/// [`read`], refusing a message over `cap` bytes: for the handshake, whose frames are tiny and
/// come before anyone is known, so a stranger cannot make a host allocate megabytes.
pub(crate) fn read_capped<T: DeserializeOwned>(input: &mut impl Read, cap: usize) -> io::Result<T> {
    let mut len = [0u8; 4];
    input.read_exact(&mut len)?;
    let len = u32::from_le_bytes(len) as usize;
    if len > cap {
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
    use serde::Deserialize;

    use super::*;

    #[derive(Debug, PartialEq, Serialize, Deserialize)]
    enum Msg {
        Pick { name: String, class: String },
    }

    #[test]
    fn a_message_survives_the_round_trip() {
        let msg = Msg::Pick {
            name: "Jaina".into(),
            class: "mage".into(),
        };
        let frame = encode(&msg).unwrap();
        let back: Msg = read(&mut frame.as_slice()).unwrap();
        assert_eq!(back, msg);
    }

    #[test]
    fn a_message_over_its_cap_is_refused() {
        let msg = Msg::Pick {
            name: "J".repeat(300),
            class: "mage".into(),
        };
        let frame = encode(&msg).unwrap();
        assert!(read_capped::<Msg>(&mut frame.as_slice(), 256).is_err());
        assert_eq!(read_capped::<Msg>(&mut frame.as_slice(), 1024).unwrap(), msg);
    }

    #[test]
    fn an_oversized_length_is_refused() {
        let frame = (u32::MAX).to_le_bytes();
        assert!(read::<Msg>(&mut frame.as_slice()).is_err());
    }
}

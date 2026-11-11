//! Format checks that read headers only. Nothing here decodes pixels or
//! audio: a pack is untrusted input, so its claims are checked before
//! any decoder sees the bytes.

pub const PNG_SIGNATURE: [u8; 8] = [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];

/// Width and height from a PNG's IHDR, or `None` if it is not a PNG.
pub fn png_size(b: &[u8]) -> Option<(u32, u32)> {
    if b.len() < 24 || b[..8] != PNG_SIGNATURE || &b[12..16] != b"IHDR" {
        return None;
    }
    let w = u32::from_be_bytes(b[16..20].try_into().ok()?);
    let h = u32::from_be_bytes(b[20..24].try_into().ok()?);
    Some((w, h))
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OggInfo {
    pub channels: u8,
    pub rate: u32,
    pub seconds: f32,
}

/// Channels, sample rate and length of an Ogg Vorbis file, or `None` if
/// the first page does not carry a Vorbis identification header. The
/// length is the last page's granule position over the rate.
pub fn ogg_vorbis_info(b: &[u8]) -> Option<OggInfo> {
    if b.len() < 28 || &b[..4] != b"OggS" {
        return None;
    }
    let start = 27 + usize::from(b[26]);
    let p = b.get(start..start + 16)?;
    if p[0] != 1 || &p[1..7] != b"vorbis" {
        return None;
    }
    let channels = p[11];
    let rate = u32::from_le_bytes(p[12..16].try_into().ok()?);
    if rate == 0 || channels == 0 {
        return None;
    }
    let last = b.windows(4).rposition(|w| w == b"OggS")?;
    let granule = u64::from_le_bytes(b.get(last + 6..last + 14)?.try_into().ok()?);
    Some(OggInfo {
        channels,
        rate,
        seconds: granule as f32 / rate as f32,
    })
}

#[cfg(test)]
pub(crate) mod fixtures {
    /// The smallest bytes `png_size` accepts: signature + IHDR header.
    pub fn png(w: u32, h: u32) -> Vec<u8> {
        let mut b = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
        b.extend_from_slice(&13u32.to_be_bytes());
        b.extend_from_slice(b"IHDR");
        b.extend_from_slice(&w.to_be_bytes());
        b.extend_from_slice(&h.to_be_bytes());
        b.extend_from_slice(&[8, 6, 0, 0, 0, 0, 0, 0, 0]);
        b
    }

    /// A first Ogg page carrying a Vorbis identification header, then a
    /// last page whose granule position is `samples`.
    pub fn ogg(channels: u8, rate: u32, samples: u64) -> Vec<u8> {
        let mut b = Vec::new();
        b.extend_from_slice(b"OggS");
        b.extend_from_slice(&[0, 2]); // version, header type (BOS)
        b.extend_from_slice(&0u64.to_le_bytes()); // granule
        b.extend_from_slice(&[0; 12]); // serial, sequence, crc
        b.push(1); // one segment
        b.push(30); // segment length
        b.push(1);
        b.extend_from_slice(b"vorbis");
        b.extend_from_slice(&0u32.to_le_bytes()); // vorbis version
        b.push(channels);
        b.extend_from_slice(&rate.to_le_bytes());
        b.extend_from_slice(&[0; 14]); // rest of the id header
        b.extend_from_slice(b"OggS");
        b.extend_from_slice(&[0, 4]); // EOS page
        b.extend_from_slice(&samples.to_le_bytes());
        b.extend_from_slice(&[0; 13]);
        b
    }
}

#[cfg(test)]
mod tests {
    use super::fixtures::{ogg, png};
    use super::*;

    #[test]
    fn png_size_reads_the_ihdr() {
        assert_eq!(png_size(&png(128, 64)), Some((128, 64)));
        assert_eq!(png_size(b"GIF89a.................."), None);
        assert_eq!(png_size(&png(1, 1)[..20]), None, "truncated");
    }

    #[test]
    fn ogg_info_reads_channels_rate_and_length() {
        let info = ogg_vorbis_info(&ogg(2, 44_100, 44_100 * 3)).unwrap();
        assert_eq!((info.channels, info.rate), (2, 44_100));
        assert!((info.seconds - 3.0).abs() < 1e-3);
        assert!(ogg_vorbis_info(b"RIFF....WAVE").is_none());
        assert!(ogg_vorbis_info(&ogg(1, 0, 10)).is_none(), "zero rate");
    }
}

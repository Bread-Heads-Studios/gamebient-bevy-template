//! Replay format `GXR1` (little-endian):
//!
//! ```text
//! magic    b"GXR1"
//! build    u8 len + UTF-8            GX_BUILD_ID of the game that recorded it
//! tick_hz  u16
//! seed     [u8; 32]
//! origin   u8                        0 host, 1 local
//! ticks    u32                       total sim ticks
//! score    u64                       claimed
//! checksum u64                       claimed
//! runs     u32                       number of RLE runs
//! run[]    held u16, latched u16, ax i8, ay i8, count u16
//! ```

pub mod feeder;
pub mod recorder;

use super::sim::SeedOrigin;

pub const MAGIC: &[u8; 4] = b"GXR1";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TickRun {
    pub held: u16,
    pub latched: u16,
    pub ax: i8,
    pub ay: i8,
    pub count: u16,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Replay {
    pub build: String,
    pub tick_hz: u16,
    pub seed: [u8; 32],
    pub origin: SeedOrigin,
    pub ticks: u32,
    pub score: u64,
    pub checksum: u64,
    pub runs: Vec<TickRun>,
}

#[derive(Debug, PartialEq, Eq)]
pub enum DecodeError {
    BadMagic,
    Truncated,
    BadOrigin(u8),
    BadBuild,
    RunCountMismatch { header: u32, sum: u32 },
}

impl Replay {
    /// Merges into the last run when `(held, latched, ax, ay)` are equal and
    /// its count hasn't saturated; else appends a new run. Always increments
    /// `ticks`.
    pub fn push_tick(&mut self, held: u16, latched: u16, ax: i8, ay: i8) {
        self.ticks += 1;
        if let Some(last) = self.runs.last_mut()
            && last.held == held
            && last.latched == latched
            && last.ax == ax
            && last.ay == ay
            && last.count < u16::MAX
        {
            last.count += 1;
            return;
        }
        self.runs.push(TickRun {
            held,
            latched,
            ax,
            ay,
            count: 1,
        });
    }

    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(64 + self.runs.len() * 8);
        out.extend_from_slice(MAGIC);
        let build = self.build.as_bytes();
        out.push(build.len().min(255) as u8);
        out.extend_from_slice(&build[..build.len().min(255)]);
        out.extend_from_slice(&self.tick_hz.to_le_bytes());
        out.extend_from_slice(&self.seed);
        out.push(self.origin as u8);
        out.extend_from_slice(&self.ticks.to_le_bytes());
        out.extend_from_slice(&self.score.to_le_bytes());
        out.extend_from_slice(&self.checksum.to_le_bytes());
        out.extend_from_slice(&(self.runs.len() as u32).to_le_bytes());
        for r in &self.runs {
            out.extend_from_slice(&r.held.to_le_bytes());
            out.extend_from_slice(&r.latched.to_le_bytes());
            out.push(r.ax as u8);
            out.push(r.ay as u8);
            out.extend_from_slice(&r.count.to_le_bytes());
        }
        out
    }

    pub fn decode(bytes: &[u8]) -> Result<Replay, DecodeError> {
        let mut c = Cursor { b: bytes, i: 0 };
        if c.take(4)? != MAGIC {
            return Err(DecodeError::BadMagic);
        }
        let n = c.u8()? as usize;
        let build = core::str::from_utf8(c.take(n)?)
            .map_err(|_| DecodeError::BadBuild)?
            .to_string();
        let tick_hz = c.u16()?;
        let mut seed = [0u8; 32];
        seed.copy_from_slice(c.take(32)?);
        let o = c.u8()?;
        let origin = SeedOrigin::from_u8(o).ok_or(DecodeError::BadOrigin(o))?;
        let ticks = c.u32()?;
        let score = c.u64()?;
        let checksum = c.u64()?;
        let n_runs = c.u32()? as usize;
        let mut runs = Vec::with_capacity(n_runs.min(1 << 16));
        let mut sum: u32 = 0;
        for _ in 0..n_runs {
            let r = TickRun {
                held: c.u16()?,
                latched: c.u16()?,
                ax: c.u8()? as i8,
                ay: c.u8()? as i8,
                count: c.u16()?,
            };
            sum = sum.saturating_add(u32::from(r.count));
            runs.push(r);
        }
        if sum != ticks {
            return Err(DecodeError::RunCountMismatch { header: ticks, sum });
        }
        Ok(Replay {
            build,
            tick_hz,
            seed,
            origin,
            ticks,
            score,
            checksum,
            runs,
        })
    }
}

struct Cursor<'a> {
    b: &'a [u8],
    i: usize,
}

impl<'a> Cursor<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8], DecodeError> {
        let end = self.i.checked_add(n).ok_or(DecodeError::Truncated)?;
        let s = self.b.get(self.i..end).ok_or(DecodeError::Truncated)?;
        self.i = end;
        Ok(s)
    }
    fn u8(&mut self) -> Result<u8, DecodeError> {
        Ok(self.take(1)?[0])
    }
    fn u16(&mut self) -> Result<u16, DecodeError> {
        Ok(u16::from_le_bytes(self.take(2)?.try_into().unwrap()))
    }
    fn u32(&mut self) -> Result<u32, DecodeError> {
        Ok(u32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }
    fn u64(&mut self) -> Result<u64, DecodeError> {
        Ok(u64::from_le_bytes(self.take(8)?.try_into().unwrap()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Replay {
        let mut r = Replay {
            build: "0.1.0+abc1234".into(),
            tick_hz: 60,
            seed: [9u8; 32],
            origin: SeedOrigin::Host,
            ticks: 0,
            score: 1500,
            checksum: 0xdead_beef,
            runs: Vec::new(),
        };
        for _ in 0..3 {
            r.push_tick(8, 0, 0, 0);
        }
        r.push_tick(8, 16, 0, 0);
        r.push_tick(0, 0, 127, -127);
        r
    }

    #[test]
    fn push_tick_run_length_encodes() {
        let r = sample();
        assert_eq!(r.ticks, 5);
        assert_eq!(r.runs.len(), 3);
        assert_eq!(
            r.runs[0],
            TickRun {
                held: 8,
                latched: 0,
                ax: 0,
                ay: 0,
                count: 3
            }
        );
        assert_eq!(r.runs[2].ax, 127);
    }

    #[test]
    fn round_trips() {
        let r = sample();
        let bytes = r.encode();
        assert_eq!(&bytes[..4], MAGIC);
        assert_eq!(Replay::decode(&bytes).unwrap(), r);
    }

    #[test]
    fn rejects_bad_input() {
        let r = sample();
        let bytes = r.encode();
        assert_eq!(Replay::decode(b"GXR0"), Err(DecodeError::BadMagic));
        assert_eq!(
            Replay::decode(&bytes[..bytes.len() - 1]),
            Err(DecodeError::Truncated)
        );
        let mut bad = bytes.clone();
        let origin_at = 4 + 1 + r.build.len() + 2 + 32;
        bad[origin_at] = 7;
        assert_eq!(Replay::decode(&bad), Err(DecodeError::BadOrigin(7)));
        let mut mism = r.clone();
        mism.ticks = 99;
        assert_eq!(
            Replay::decode(&mism.encode()),
            Err(DecodeError::RunCountMismatch { header: 99, sum: 5 })
        );
    }

    #[test]
    fn run_count_saturates_at_u16() {
        let mut r = sample();
        r.runs.clear();
        r.ticks = 0;
        for _ in 0..70_000 {
            r.push_tick(1, 0, 0, 0);
        }
        assert_eq!(r.ticks, 70_000);
        assert_eq!(r.runs.len(), 2);
        assert_eq!(r.runs[0].count, u16::MAX);
    }
}

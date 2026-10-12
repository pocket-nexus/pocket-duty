//! The reference's random numbers (`web/src/core/rng.js`): xoshiro128** on
//! wrapping 32-bit words, seeded through one mixing function. The same seed
//! gives the same match on every machine.

#[derive(Clone, Copy, Debug)]
pub struct Rng {
    s: [u32; 4],
}

impl Rng {
    pub fn new(seed: u32) -> Rng {
        let mut z = seed;
        let mut next = || {
            z = z.wrapping_add(0x9e37_79b9);
            let mut x = z;
            x = (x ^ (x >> 16)).wrapping_mul(0x21f0_aaad);
            x = (x ^ (x >> 15)).wrapping_mul(0x735a_2d97);
            x ^ (x >> 15)
        };
        Rng {
            s: [next(), next(), next(), next()],
        }
    }

    pub fn u32(&mut self) -> u32 {
        let s = &mut self.s;
        let r = s[1].wrapping_mul(5).rotate_left(7).wrapping_mul(9);
        let t = s[1] << 9;
        s[2] ^= s[0];
        s[3] ^= s[1];
        s[1] ^= s[2];
        s[0] ^= s[3];
        s[2] ^= t;
        s[3] = s[3].rotate_left(11);
        r
    }

    /// 0 ≤ x < 1.
    pub fn float(&mut self) -> f32 {
        (self.u32() >> 8) as f32 * (1.0 / 16_777_216.0)
    }

    /// -1 ≤ x < 1.
    pub fn signed(&mut self) -> f32 {
        self.float() * 2.0 - 1.0
    }

    pub fn range(&mut self, lo: f32, hi: f32) -> f32 {
        lo + (hi - lo) * self.float()
    }

    /// `lo` to `hi`, both included.
    pub fn int(&mut self, lo: u32, hi: u32) -> u32 {
        lo + self.u32() % (hi - lo + 1)
    }

    /// A stream of its own, started from one draw of this one.
    pub fn fork(&mut self) -> Rng {
        Rng::new(self.u32())
    }

    /// About normal, mean 0 and deviation 1: the sum of four draws.
    pub fn gauss(&mut self) -> f32 {
        (self.float() + self.float() + self.float() + self.float() - 2.0) * 1.732
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_reference_seed_gives_the_reference_numbers() {
        // The capture seed's state and its first draws, as web/src/core/rng.js produces them.
        let mut r = Rng::new(0x5eed_1234);
        assert_eq!(r.s, [0x53f7_617d, 0x79c1_94a2, 0x0fcc_8be1, 0x7878_7b99]);
        assert_eq!(
            [r.u32(), r.u32(), r.u32()],
            [0x8390_3eb0, 0x8418_764e, 0x9f57_0b4d]
        );
    }
}

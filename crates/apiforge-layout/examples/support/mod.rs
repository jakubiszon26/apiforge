//! What the layout examples share. Each one uses its part of it.
#![allow(dead_code)]

/// The same small generator the tests use, so the boards are comparable.
pub struct Random(pub u64);

impl Random {
    pub fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    pub fn below(&mut self, limit: usize) -> usize {
        if limit == 0 {
            0
        } else {
            (self.next() % limit as u64) as usize
        }
    }

    pub fn float(&mut self, low: f32, high: f32) -> f32 {
        let unit = (self.next() >> 11) as f32 / (1u64 << 53) as f32;
        low + (high - low) * unit
    }

    pub fn chance(&mut self, percent: u64) -> bool {
        self.next() % 100 < percent
    }
}

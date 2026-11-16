//! NGP's loaded-value accumulator, `Crc32` (7.5.779 `0x100e8a90`-`0x100e8b00`).
//!
//! The constructor seeds `!0` (through the 32-bit bit reversal at `0x10015990`, which leaves `!0` unchanged) and
//! stores the walk mode at `+4`; the getter `0x100e8ae0` returns `!value`. The update helpers (`0x10016fb0`,
//! `0x10042a90`) fold bytes as `value = value >> 8 ^ table[(value ^ byte) & 0xff]` and skip the loop when the
//! length is zero. The table (`[0x10188d50]` points at `0x10188d58`, built once under the guard `0x10188d54` by
//! `0x100158b0` with poly `0x04c11db7`, 32 bits and the reflected flag) is the standard CRC-32 table; the
//! reconstruction in the tests runs the original builder's steps.
//!
//! This is only the accumulator. The walk that feeds it (`0x100418c0` and its callees: the loaded car definition,
//! the registry's body parts and wheels, the car body's suspension record and the setup options) is **not ported**,
//! so no `CarId` can be reproduced yet and NGP 7.5 stays disabled.

/// The mode init passes at `0x100411bf` (the loaded car and options).
pub const INIT_MODE: u32 = 1;
/// The mode the runtime recheck sets (`0x100e8b00`); it also folds six globals the init walk does not.
pub const RECHECK_MODE: u32 = 2;

/// The `Crc32` object at `[0x100e8a90]`: the running value and the walk mode.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Crc32 {
    value: u32,
    mode: u32,
}

impl Crc32 {
    /// `0x100e8a90`: the value starts as `!0`, the mode is the constructor's argument.
    pub const fn new(mode: u32) -> Self {
        Self { value: !0, mode }
    }

    /// `0x10016fb0` / `0x10042a90`: fold each byte; a zero length leaves the value alone.
    pub fn update(&mut self, bytes: &[u8]) {
        for &byte in bytes {
            let index = ((self.value ^ u32::from(byte)) & 0xff) as usize;
            self.value = self.value >> 8 ^ TABLE[index];
        }
    }

    /// `0x100e8ae0`: the complemented value, the word the init compares with the car's `CarId`.
    pub const fn finish(&self) -> u32 {
        !self.value
    }

    /// `0x100e8af0`.
    pub const fn mode(&self) -> u32 {
        self.mode
    }

    /// `0x100e8b00`.
    pub fn set_mode(&mut self, mode: u32) {
        self.mode = mode;
    }
}

/// The table `0x100158b0` builds into `0x10188d58`: CRC-32, poly `0x04c11db7`, reflected.
const TABLE: [u32; 256] = {
    let mut table = [0; 256];
    let mut index = 0;
    while index < 256 {
        let mut crc = index as u32;
        let mut bit = 0;
        while bit < 8 {
            crc = if crc & 1 == 0 { crc >> 1 } else { crc >> 1 ^ 0xedb8_8320 };
            bit += 1;
        }
        table[index] = crc;
        index += 1;
    }
    table
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn check_value() {
        let mut crc = Crc32::new(INIT_MODE);
        crc.update(b"123456789");
        assert_eq!(crc.finish(), 0xcbf4_3926);
    }

    #[test]
    fn empty_input_finishes_as_zero() {
        let mut crc = Crc32::new(INIT_MODE);
        crc.update(&[]);
        assert_eq!(crc.finish(), 0);
    }

    #[test]
    fn updates_are_the_same_split_anywhere() {
        let bytes = b"the loaded car, its parts and the setup options";
        let mut whole = Crc32::new(INIT_MODE);
        whole.update(bytes);
        for split in 0..=bytes.len() {
            let mut split_crc = Crc32::new(INIT_MODE);
            split_crc.update(&bytes[..split]);
            split_crc.update(&bytes[split..]);
            assert_eq!(split_crc, whole, "split at {split}");
        }
    }

    #[test]
    fn mode_is_stored_and_returned() {
        let mut crc = Crc32::new(INIT_MODE);
        assert_eq!(crc.mode(), INIT_MODE);
        crc.set_mode(RECHECK_MODE);
        assert_eq!(crc.mode(), RECHECK_MODE);
    }

    /// `0x10015810` (entry) and `0x100158b0` (table): the index is bit-reversed for the MSB-first loop, the entry
    /// is bit-reversed for the 32-bit table, and the result is stored at the bit-reversed index. The product is the
    /// standard reflected table above.
    #[test]
    fn table_reconstructs_the_original_builder() {
        for index in 0..=255u8 {
            assert_eq!(built_entry(index), TABLE[usize::from(index.reverse_bits())], "index {index:#04x}");
        }
    }

    fn built_entry(index: u8) -> u32 {
        let mut entry = 0u32;
        let mut bits = index.reverse_bits();
        for _ in 0..8 {
            let top = 0x8000_0000;
            let mixed = entry ^ if bits & 1 == 0 { 0 } else { top };
            entry = mixed << 1 ^ if mixed & top == 0 { 0 } else { 0x04c1_1db7 };
            bits >>= 1;
        }
        entry.reverse_bits()
    }
}

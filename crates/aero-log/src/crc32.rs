//! A small CRC-32 (IEEE 802.3 polynomial) used for per-record integrity in the
//! observation log. Implemented here rather than pulled from a crate so the log
//! format has no external dependency and the checksum is auditable; the one
//! known test vector below pins it to the standard polynomial.

/// Precomputed table for the reflected IEEE polynomial 0xEDB88320.
struct Crc32Table([u32; 256]);

impl Crc32Table {
    const fn new() -> Self {
        let mut table = [0u32; 256];
        let mut i = 0;
        while i < 256 {
            let mut crc = i as u32;
            let mut j = 0;
            while j < 8 {
                if crc & 1 == 1 {
                    crc = (crc >> 1) ^ 0xEDB88320;
                } else {
                    crc >>= 1;
                }
                j += 1;
            }
            table[i] = crc;
            i += 1;
        }
        Self(table)
    }
}

static TABLE: Crc32Table = Crc32Table::new();

/// Compute the CRC-32 of a byte slice.
pub fn crc32(data: &[u8]) -> u32 {
    let mut crc = 0xFFFF_FFFFu32;
    for &b in data {
        let idx = ((crc ^ b as u32) & 0xFF) as usize;
        crc = (crc >> 8) ^ TABLE.0[idx];
    }
    crc ^ 0xFFFF_FFFF
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_vector() {
        // Standard check value for "123456789" is 0xCBF43926.
        assert_eq!(crc32(b"123456789"), 0xCBF4_3926);
    }

    #[test]
    fn empty_is_zero() {
        assert_eq!(crc32(b""), 0);
    }

    #[test]
    fn detects_single_bit_change() {
        let a = crc32(b"hello world");
        let b = crc32(b"hello worle");
        assert_ne!(a, b);
    }
}

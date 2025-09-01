//! Ported from https://github.com/meza/curseforge-fingerprint/blob/b15012c026c56ca89fad90f8cf9a8e140616e2c0/src/addon/fingerprint.cpp
#![allow(clippy::let_and_return)]
pub struct MurmurHash2;

const MULTIPLEX: u32 = 1540483477;

impl MurmurHash2 {
    pub fn hash(data: &[u8]) -> u32 {
        let n_length = MurmurHash2::normalise(data);
        let mut seed = 1_u32 ^ n_length;
        let mut num_1 = 0;
        let mut num_2 = 0;
        for c in data.iter().filter(|&c| !is_whitespace(*c)) {
            num_1 |= (*c as u32) << num_2;
            num_2 += 8;
            if num_2 == 32 {
                seed = seed.wrapping_mul(MULTIPLEX) ^ MurmurHash2::mix(num_1);
                num_1 = 0;
                num_2 = 0;
            }
        }
        if num_2 > 0 {
            seed = (seed ^ num_1).wrapping_mul(MULTIPLEX);
        }
        let hash = (seed ^ (seed >> 13)).wrapping_mul(MULTIPLEX);
        hash ^ (hash >> 15)
    }
    #[inline]
    const fn mix(num_1: u32) -> u32 {
        let num_3 = num_1.wrapping_mul(MULTIPLEX);
        let num_4 = (num_3 ^ (num_3 >> 24)).wrapping_mul(MULTIPLEX);
        num_4
    }
    fn normalise(data: &[u8]) -> u32 {
        let mut n_len = 0;
        data.iter()
            .filter(|&c| !is_whitespace(*c))
            .for_each(|_| n_len += 1);
        n_len
    }
}

fn is_whitespace(c: u8) -> bool {
    c == b' ' || c == b'\t' || c == b'\r' || c == b'\n'
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    #[test]
    fn test_murmur_hash() {
        let file = "Hello world";
        let hash = MurmurHash2::hash(file.as_bytes());
        assert_eq!(hash, 1423925525);
    }
}

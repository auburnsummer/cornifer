use std::{collections::HashMap, hash::BuildHasherDefault};

use nohash_hasher::{IntMap, NoHashHasher};

const MAX_BITS: u16 = 15;

#[derive(PartialEq)]
pub struct HuffmanTree {
    lut: HashMap<u16, HuffmanCode, BuildHasherDefault<NoHashHasher<u16>>>
}

#[derive(Copy, Clone, PartialEq, Debug)]
pub struct HuffmanCode {
    pub symbol: u16,
    pub len: u8
}

impl HuffmanTree {
    pub fn new(bit_lengths: &[u8]) -> Self {
        // https://www.rfc-editor.org/rfc/rfc1951
        // Count the number of codes for each code length.  Let
        // bl_count[N] be the number of codes of length N, N >= 1.
        let mut bl_count = [0_u16; (MAX_BITS + 1) as usize];
        let mut next_code = [0_u16; (MAX_BITS + 1) as usize];
        for len in bit_lengths {
            let len = *len as usize;
            bl_count[len] += 1
        }
        // 2)  Find the numerical value of the smallest code for each
        // code length:
        let mut code: u16 = 0;
        for bits in 1..=MAX_BITS {
            let bits = bits as usize;
            code = (code + bl_count[bits - 1]) << 1;
            next_code[bits] = code;
        }
        // Assign numerical values to all codes.
        let mut final_codes = vec![0_u16; bit_lengths.len()];
        for (i, &len) in bit_lengths.iter().enumerate() {
            if len != 0 {
                let len = len as usize;
                let code = next_code[len];
                next_code[len] += 1;
                final_codes[i] = code;
            } else {
                final_codes[i] = 0;
            }
        }
        // put them in the lookup table.
        let mut lut: IntMap<u16, HuffmanCode> = IntMap::default();
        for i in 0..bit_lengths.len() {
            let len = bit_lengths[i];
            let code = final_codes[i];
            let i = i as u16;
            lut.insert(code, HuffmanCode {
                symbol: i,
                len
            });
        }

        Self {
            lut
        }
    }

    pub fn fixed() -> Self {
        let mut test_values: Vec<u8> = vec![];
        for (next, bit_len) in [
            (143, 8),
            (255, 9),
            (279, 7),
            (287, 8)
        ] {
            test_values.resize(next + 1, bit_len);
        }
        Self::new(&test_values)
    }

    pub fn decode(&self, code: u16, len: u8) -> Option<u16> {
        let lookup = self.lut.get(&code)?;
        if len == lookup.len {
            Some(lookup.symbol)
        } else {
            None
        }
    }

    #[cfg(test)]
    pub fn get_lut(&self) -> &HashMap<u16, HuffmanCode, BuildHasherDefault<NoHashHasher<u16>>> {
        return &self.lut;
    }

    pub fn export(&self) {
        
    }
}


/**
 * TESTS
 */
#[cfg(test)]
mod test {
    use rstest::*;

    use crate::huffman::HuffmanCode;

    use super::HuffmanTree;


    #[rstest]
    pub fn test_lut_values_correct() {
        let test_values: [u8; 8] = [3, 3, 3, 3, 3, 2, 4, 4];
        let tree = HuffmanTree::new(&test_values);

        let codes = tree.get_lut();

        /*
            Symbol Length   Code
            ------ ------   ----
            A       3        010
            B       3        011
            C       3        100
            D       3        101
            E       3        110
            F       2         00
            G       4       1110
            H       4       1111
         */
        assert_eq!(codes.get(&0b01), None);
        assert_eq!(codes.get(&0b010), Some(&HuffmanCode {
            symbol: 0,
            len: 3
        }));
        assert_eq!(codes.get(&0b1111), Some(&HuffmanCode {
            symbol: 7,
            len: 4
        }));
        assert_eq!(codes.get(&0b00), Some(&HuffmanCode {
            symbol: 5,
            len: 2
        }));
    }

    #[rstest]
    pub fn test_lut_fixed() {
        let tree = HuffmanTree::fixed();
        let codes = tree.get_lut();
        /*
        Lit Value    Bits        Codes
        ---------    ----        -----
          0 - 143     8          00110000 through
                                10111111
        144 - 255     9          110010000 through
                                111111111
        256 - 279     7          0000000 through
                                0010111
        280 - 287     8          11000000 through
                                11000111
         */
        assert_eq!(codes.get(&0b110001), Some(&HuffmanCode {
            symbol: 1,
            len: 8
        }));
        assert_eq!(codes.get(&0b11000111), Some(&HuffmanCode {
            symbol: 287,
            len: 8
        }));
        assert_eq!(codes.get(&0b111111110), Some(&HuffmanCode {
            symbol: 254,
            len: 9
        }));
        assert_eq!(codes.get(&0b0000000), Some(&HuffmanCode {
            symbol: 256,
            len: 7
        }));
        assert_eq!(codes.get(&0b1111111111), None);
    }

    #[rstest]
    pub fn test_decode() {
        let test_values: [u8; 8] = [3, 3, 3, 3, 3, 2, 4, 4];
        let tree = HuffmanTree::new(&test_values);
        assert_eq!(tree.decode(0b0, 1), None);
        assert_eq!(tree.decode(0b10, 2), None);
        assert_eq!(tree.decode(0b010, 3), Some(0));
    }
}
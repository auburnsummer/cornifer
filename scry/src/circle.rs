use std::mem;

use crc::{CRC_32_ISO_HDLC, Crc, Digest};
use rand::Rng;

use crate::errors::ScryError;

static CRC32: Crc<u32> = Crc::<u32>::new(&CRC_32_ISO_HDLC);

pub struct CircularBuffer {
    buffer: Vec<u8>,
    head: usize,
    digest: Digest<'static, u32>,
    counter: u32
}

impl CircularBuffer {
    pub fn new(size: usize) -> Self {
        let mut rng = rand::thread_rng();
        let buffer: Vec<u8> = vec![0; size];
        Self {
            buffer,
            head: rng.gen_range(0..size), // it shouldn't matter where the head starts.
            digest: CRC32.digest(),
            counter: 0
        }
    }

    pub fn push(&mut self, byte: u8) {
        self.buffer[self.head] = byte;
        self.head = (self.head + 1) % self.buffer.len();
        self.digest.update(&[byte]);
        self.counter += 1;
    }

    /// push bytes into the buffer that are in the buffer.
    /// 
    ///  * lookback - number of bytes back in the buffer to look. Max 32kb.
    ///  * size - number of bytes _from_ lookback to start copying.
    /// 
    /// Note that size can be greater than lookback, because as a byte is copied into the
    /// buffer, it can be read again as input.  
    pub fn push_from_buffer(&mut self, lookback: u16, size: u16) -> Result<(), ScryError> {
        if lookback > 32768 {
            return Err(ScryError::InvalidLengthDistancePair { lookback, size });
        }
        let lookback = lookback as isize;
        let len = self.buffer.len() as isize;
        for _ in 0..size {
            let head = self.head as isize;
            let target = (head - lookback).rem_euclid(len) as usize;
            self.push(self.buffer[target]);
        }
        Ok(())
    }

    /// Get the top n bytes of the buffer as a vector v.
    /// The _last_ item in v is the most _recent_ byte pushed to the buffer.
    /// The _first_ item in v is the nth most recent byte pushed to the buffer.
    pub fn head(&self, n: u16) -> Result<Vec<u8>, ScryError> {
        let mut v: Vec<u8> = Vec::new();
        for i in 0..n {
            let n1 = (n - i) as isize;
            let head = self.head as isize;
            let len = self.buffer.len() as isize;
            let index = (head - n1).rem_euclid(len);
            v.push(self.buffer[index as usize])
        }

        Ok(v)
    }

    /// Returns the CRC32 of the data written so far, and resets the CRC32.
    pub fn crc32(&mut self) -> u32 {
        let d = mem::replace(&mut self.digest, CRC32.digest());
        d.finalize()
    }

    /// Return the number of bytes written so far, and resets this count.
    pub fn counter(&mut self) -> u32 {
        let result = self.counter;
        self.counter = 0;
        result
    }

    #[cfg(test)]
    pub fn get_normalized_buffer(&self) -> Vec<u8> { 
        self.head(self.buffer.len() as u16).unwrap()
    }
}

#[cfg(test)]
mod test {
    use rstest::*;

    use crate::circle::CircularBuffer;

    #[rstest]
    pub fn test_get_normalized_buffer() {
        let mut cb = CircularBuffer::new(8);
        for _ in 0..5 {
            for i in 0..8 {
                cb.push(i);
            }
            let nb = cb.get_normalized_buffer();
            assert_eq!(vec![0, 1, 2, 3, 4, 5, 6, 7], nb);
        }
    }

    #[rstest]
    pub fn test_get_normalized_buffer_overwrite() {
        let mut cb = CircularBuffer::new(8);
        for i in 0..9 {
            cb.push(i);
        }
        let nb = cb.get_normalized_buffer();
        assert_eq!(vec![1, 2, 3, 4, 5, 6, 7, 8], nb);
    }

    #[rstest]
    pub fn test_push_from_buffer() {
        let mut cb = CircularBuffer::new(8);
        for i in 0..8 {
            cb.push(i); // cb is [0, 1, 2, 3, 4, 5, 6, 7]
        }
        cb.push_from_buffer(5, 3).unwrap();
        // [0, 1, 2, 3, 4, 5, 6, 7]
        // we should go back 5 and write 3
        // which is [3, 4, 5]
        // so it should look like
        let expected: Vec<u8> = vec![3, 4, 5, 6, 7, 3, 4, 5];
        assert_eq!(cb.get_normalized_buffer(), expected);
    }

    #[rstest]
    pub fn test_push_from_buffer_rle() {
        let mut cb = CircularBuffer::new(800);
        cb.push(3);
        cb.push_from_buffer(1, 799).unwrap();
        let expected: Vec<u8> = vec![3; 800];
        assert_eq!(cb.get_normalized_buffer(), expected);
    }

    #[rstest]
    pub fn test_head() {
        let mut cb = CircularBuffer::new(8);
        for i in 0..8 {
            cb.push(i); // cb is [0, 1, 2, 3, 4, 5, 6, 7]
        }
        let v = cb.head(5).unwrap();
        assert_eq!(v, vec![3, 4, 5, 6, 7]);
    }
}

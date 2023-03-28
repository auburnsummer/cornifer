/*
Circular buffer.
 */

use rand::Rng;

use std::iter::zip;

use crate::errors::ScryError;

pub struct CircularBuffer {
    buffer: Vec<u8>,
    head: usize,
}

impl CircularBuffer {
    pub fn new(size: usize) -> Self {
        let mut rng = rand::thread_rng();
        let buffer: Vec<u8> = vec![0; size];
        Self {
            buffer,
            head: rng.gen_range(0..size), // it shouldn't matter where the head starts.
        }
    }

    pub fn push(&mut self, byte: u8) {
        self.buffer[self.head] = byte;
        self.head = (self.head + 1) % self.buffer.len()
    }

    pub fn get_normalized_buffer(&self) -> Vec<u8> {
        let mut buffer2 = vec![0; self.buffer.len()];
        /*       a0------->a1---------------------a2--------------->a3
        buffer1  |==================================================|
        buffer2  |==================================================|
        copy a1->a3 to a0->a2
        copy a0->a1 to a2->a3
         */

        let a0 = 0_usize;
        let a1 = self.head;
        let a2 = self.buffer.len() - self.head;
        let a3 = self.buffer.len();

        for (k1, k2) in zip(a1..a3, a0..a2) {
            buffer2[k2] = self.buffer[k1];
        }

        for (k1, k2) in zip(a0..a1, a2..a3) {
            buffer2[k2] = self.buffer[k1];
        }

        buffer2
    }

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

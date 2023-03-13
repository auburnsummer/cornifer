/*
Circular buffer.
 */

use rand::Rng;

use std::iter::zip;

use crate::errors::ScryError;


pub struct CircularBuffer {
    buffer: Vec<u8>,
    head: usize
}

impl CircularBuffer {
    pub fn new(size: usize) -> Self {
        let mut rng = rand::thread_rng();
        let buffer: Vec<u8> = vec![0; size];
        Self {
            buffer,
            head: rng.gen_range(0..size) // it shouldn't matter where the head starts.
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
        };

        for (k1, k2) in zip(a0..a1, a2..a3) {
            buffer2[k2] = self.buffer[k1];
        }

        buffer2
    }

    pub fn push_from_buffer(&mut self, lookback: u16, size: u16) -> Result<(), ScryError> {
        if lookback == 0 || lookback > 32768 || size < 3 || size > 258 {
            return Err(ScryError::InvalidLengthDistancePair { lookback, size })
        }
        let mut vec: Vec<u8> = vec![0; size.into()];
        for i in 0..size {
            let i = i as isize;
            let head = self.head as isize;
            let len = self.buffer.len() as isize;
            let lookback = lookback as isize;
            let target_byte = (head - lookback - i).rem_euclid(len) as usize;
            vec[i as usize] = self.buffer[target_byte];
        }
        for byte in vec {
            self.push(byte);
        }

        Ok(())
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
    pub fn test_push_from_buffer() {
        let mut cb = CircularBuffer::new(8);
        for i in 0..8 {
            cb.push(i); // cb is [0, 1, 2, 3, 4, 5, 6, 7]
        }
        cb.push_from_buffer(5, 3).unwrap();
        // [0, 1, 2, 3, 4, 5, 6, 7]
        // we should go back 5 and write 3
        // which is [2, 3, 4]
        // so it should look like
        let expected: Vec<u8> = vec![3, 4, 5, 6, 7, 2, 3, 4];
        assert_eq!(cb.get_normalized_buffer(), expected);
    }
}
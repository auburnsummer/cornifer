use std::io::Read;

use crc::{Crc, Digest, CRC_32_ISO_HDLC};

use crate::errors::CorniferError;

static CRC32: Crc<u32> = Crc::<u32>::new(&CRC_32_ISO_HDLC);

pub struct CorniferByteReader<R> {
    // where we are in the file.
    pub current_byte: usize,
    pub current_bit: u8,
    // the current byte, for use when reading individual bits.
    buffer: u8,
    // reference to internal reader. This has ownership over the reader;
    // once it's passed to this, there's no getting it back.
    inner: R,
    // a crc32 digest. The crc object is static.
    digest: Option<Digest<'static, u32>>,
}

impl<R: Read> CorniferByteReader<R> {
    pub fn new(reader: R) -> Self {
        Self {
            current_byte: 0,
            current_bit: 0,
            buffer: 0,
            inner: reader,
            digest: None,
        }
    }

    fn read_exact_internal(&mut self, buf: &mut [u8]) -> Result<(), CorniferError> {
        let l = buf.len();
        match self.inner.read_exact(buf) {
            Ok(_) => (),
            Err(e) => match e.kind() {
                std::io::ErrorKind::UnexpectedEof => return Err(CorniferError::EOF),
                _ => return Err(CorniferError::from(e)),
            },
        }
        if let Some(digest) = &mut self.digest {
            digest.update(buf);
        }
        self.current_byte += l;

        Ok(())
    }

    pub fn read_u8(&mut self) -> Result<u8, CorniferError> {
        let mut buffer: [u8; 1] = [0; 1];
        self.read_exact_internal(&mut buffer)?;

        Ok(buffer[0])
    }

    pub fn read_u16_le(&mut self) -> Result<u16, CorniferError> {
        let mut buffer: [u8; 2] = [0; 2];
        self.read_exact_internal(&mut buffer)?;

        Ok(u16::from_le_bytes(buffer))
    }

    pub fn read_u32_le(&mut self) -> Result<u32, CorniferError> {
        let mut buffer: [u8; 4] = [0; 4];
        self.read_exact_internal(&mut buffer)?;

        Ok(u32::from_le_bytes(buffer))
    }

    pub fn read_null_terminated_string(&mut self) -> Result<String, CorniferError> {
        let mut v: Vec<u8> = vec![];
        loop {
            match self.read_u8()? {
                0 => break,
                i => v.push(i),
            }
        }
        let s = String::from_utf8(v)?;

        Ok(s)
    }

    pub fn begin_crc(&mut self) {
        let digest = CRC32.digest();
        self.digest = Some(digest);
    }

    pub fn end_crc(&mut self) -> Option<u32> {
        let result = self.digest.take();
        result.map(|d| d.finalize())
    }

    pub fn read_bit(&mut self) -> Result<u8, CorniferError> {
        if self.current_bit == 0 {
            self.buffer = self.read_u8()?;
        }
        let result = (self.buffer >> self.current_bit) & 1;
        self.current_bit = (self.current_bit + 1) % 8;
        Ok(result)
    }

    pub fn read_n_bits_le(&mut self, n: u8) -> Result<u16, CorniferError> {
        if n > 16 {
            return Err(CorniferError::InvalidNumberOfBits { num: n });
        }
        let mut value: u16 = 0;
        for i in 0..n {
            let next_bit = self.read_bit()? as u16;
            value |= next_bit << i;
        }
        Ok(value)
    }

    pub fn discard_until_next_byte(&mut self) {
        // the next call to read_bit() will read another byte, thus
        // discarding any leftover bits in the current byte.
        self.current_bit = 0;
    }
}

/**
 * TESTS
 */
#[cfg(test)]
mod test {
    use rstest::*;

    use super::CorniferByteReader;

    #[fixture]
    pub fn reader1() -> CorniferByteReader<&'static [u8]> {
        let inner: &[u8] = &[5, 6, 7, 0, 1, 2, 3, 4];
        let sr = CorniferByteReader::new(inner);
        sr
    }

    #[rstest]
    pub fn test_read_u8(mut reader1: CorniferByteReader<&'static [u8]>) {
        let res = reader1.read_u8().expect("Fixture will always have value");
        assert_eq!(res, 5);
        assert_eq!(reader1.current_byte, 1);
        let res = reader1.read_u8().expect("Fixture will always have value");
        assert_eq!(res, 6);
        assert_eq!(reader1.current_byte, 2);
    }

    #[rstest]
    pub fn test_read_u16_le(mut reader1: CorniferByteReader<&'static [u8]>) {
        let res = reader1
            .read_u16_le()
            .expect("Fixture will always have value");
        // 5 6
        // 0x0605
        assert_eq!(res, 0x0605);
        assert_eq!(reader1.current_byte, 2);
    }

    #[rstest]
    pub fn test_read_u32_le(mut reader1: CorniferByteReader<&'static [u8]>) {
        let resk = reader1
            .read_u32_le()
            .expect("Fixture will always have value");
        // 5 6 7 0
        // LE: 0 7 6 5
        // = 0x00070605
        assert_eq!(resk, 0x00070605);
        assert_eq!(reader1.current_byte, 4);
    }

    #[rstest]
    pub fn test_read_null_terminated_string() {
        let inner: &[u8] = &[
            b'h', b'e', b'l', b'l', b'o', b' ', b'w', b'o', b'r', b'l', b'd', 0,
        ];
        let mut sr = CorniferByteReader::new(inner);
        let s = sr.read_null_terminated_string().expect("Known value");
        assert_eq!(s, "hello world");
        assert_eq!(sr.current_byte, 12)
    }

    #[rstest]
    pub fn test_crc32_initial_value() {
        let inner: &[u8] = &[];
        let mut sr = CorniferByteReader::new(inner);
        sr.begin_crc();
        let result = sr.end_crc().expect("should have value");
        assert_eq!(result, 0x0000);
    }

    #[rstest]
    pub fn test_crc32_one_byte() {
        let inner: &[u8] = &[b'h'];
        let mut sr = CorniferByteReader::new(inner);
        sr.begin_crc();
        sr.read_u8().expect("known value");
        let result = sr.end_crc().expect("should have value");
        assert_eq!(result, 0x916B06E7);
    }

    #[rstest]
    pub fn test_crc32() {
        let inner: &[u8] = &[b'h', b'e', b'l', b'l', b'o'];
        let mut sr = CorniferByteReader::new(inner);
        sr.begin_crc();
        for _ in 0..inner.len() {
            sr.read_u8().expect("known value");
        }
        let result = sr.end_crc().expect("should have value");
        assert_eq!(result, 0x3610A686);
    }

    #[rstest]
    pub fn test_crc32_long() {
        let inner: &[u8] = include_bytes!("../testfiles/testCompressThenConcat.txt.gz");
        let mut sr = CorniferByteReader::new(inner);
        sr.begin_crc();
        for _ in 0..inner.len() {
            sr.read_u8().expect("known value");
        }
        let result = sr.end_crc().expect("should have value");
        assert_eq!(result, 0xFFDFCA91);
    }

    #[rstest]
    pub fn test_read_bit() {
        let inner: &[u8] = &[0b10011001, 0b00011100];
        let mut sr = CorniferByteReader::new(inner);
        assert_eq!(sr.current_byte, 0);

        assert_eq!(sr.read_bit().unwrap(), 1);

        assert_eq!(sr.current_byte, 1);

        assert_eq!(sr.read_bit().unwrap(), 0);
        assert_eq!(sr.current_bit, 2);
        assert_eq!(sr.read_bit().unwrap(), 0);
        assert_eq!(sr.read_bit().unwrap(), 1);
        assert_eq!(sr.read_bit().unwrap(), 1);
        assert_eq!(sr.read_bit().unwrap(), 0);
        assert_eq!(sr.read_bit().unwrap(), 0);
        assert_eq!(sr.read_bit().unwrap(), 1);
        assert_eq!(sr.current_bit, 0);

        assert_eq!(sr.read_bit().unwrap(), 0);

        assert_eq!(sr.current_byte, 2);

        assert_eq!(sr.read_bit().unwrap(), 0);
        assert_eq!(sr.read_bit().unwrap(), 1);
        assert_eq!(sr.read_bit().unwrap(), 1);
        assert_eq!(sr.read_bit().unwrap(), 1);
        assert_eq!(sr.read_bit().unwrap(), 0);
        assert_eq!(sr.read_bit().unwrap(), 0);
        assert_eq!(sr.read_bit().unwrap(), 0);
    }

    #[rstest]
    pub fn test_read_n_bits() {
        let inner: &[u8] = &[0b10011001, 0b00011100];
        let mut sr = CorniferByteReader::new(inner);
        assert_eq!(sr.read_n_bits_le(2).unwrap(), 0b01);
        assert_eq!(sr.read_n_bits_le(2).unwrap(), 0b10);
        assert_eq!(sr.read_n_bits_le(2).unwrap(), 0b01);
    }
}

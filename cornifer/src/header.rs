use std::io::Read;

use crate::{errors::CorniferError, reader::CorniferByteReader};

#[derive(PartialEq, Debug)]
pub struct GzipHeader {
    text: bool,
    name: Option<String>,
    comment: Option<String>,
    mtime: u32,
    extra: ExtraFlag,
    os: OperatingSystem,
}

#[derive(PartialEq, Debug)]
pub enum ExtraFlag {
    SlowestAlgorithm,
    FastestAlgorithm,
    Unknown,
}

#[derive(PartialEq, Debug)]
pub enum OperatingSystem {
    Fat,
    Unix,
    Macintosh,
    NTFS,
    Unknown, // rest not included
}

/**
 * Read a Header struct out of a corniferReader
 */
pub fn read_header<R: Read>(sr: &mut CorniferByteReader<R>) -> Result<GzipHeader, CorniferError> {
    sr.begin_crc();
    // id1 and id2
    // btw if the first byte fails, we handle that differently, it might be an
    // expected EOF
    let id1 = match sr.read_u8() {
        Ok(byte) => byte,
        Err(err) => match err {
            CorniferError::EOF => return Err(CorniferError::ExpectedEOF),
            _ => return Err(err),
        },
    };
    let id2 = sr.read_u8()?;
    if id1 != 0x1f || id2 != 0x8b {
        return Err(CorniferError::NotGZIPHeader);
    }
    // cm
    let cm = sr.read_u8()?;
    if cm != 8 {
        return Err(CorniferError::InvalidCompressionMethod);
    }
    // flgs
    let flg = sr.read_u8()?;
    let ftext = flg & 1;
    let fhcrc = (flg >> 1) & 1;
    let fextra = (flg >> 2) & 1;
    let fname = (flg >> 3) & 1;
    let fcomment = (flg >> 4) & 1;

    // mtime
    let mtime = sr.read_u32_le()?;

    // xfl
    let xfl = match sr.read_u8()? {
        2 => ExtraFlag::SlowestAlgorithm,
        4 => ExtraFlag::FastestAlgorithm,
        _ => ExtraFlag::Unknown,
    };

    // os
    let os = match sr.read_u8()? {
        0 => OperatingSystem::Fat,
        3 => OperatingSystem::Unix,
        7 => OperatingSystem::Macintosh,
        11 => OperatingSystem::NTFS,
        _ => OperatingSystem::Unknown,
    };

    // if fextra set...
    if fextra == 1 {
        // read two bytes, this is the length of the extra data.
        let xlen = sr.read_u16_le()?;
        // read and discard, we're not using the extra data for now.
        for _ in 0..xlen {
            sr.read_u8()?;
        }
    }
    // if fname set...
    let name = match fname {
        1 => Some(sr.read_null_terminated_string()?),
        _ => None,
    };
    // if fcomment set...
    let comment = match fcomment {
        1 => Some(sr.read_null_terminated_string()?),
        _ => None,
    };
    let hcrc_actual = sr.end_crc().expect("Header always should exist");
    if fhcrc == 1 {
        let truncated = hcrc_actual as u16;
        let hcrc = sr.read_u16_le()?;
        if hcrc != truncated {
            return Err(CorniferError::InvalidHeaderCRC {
                expected: truncated,
                found: hcrc,
            });
        }
    }

    Ok(GzipHeader {
        text: ftext == 1,
        name,
        comment,
        mtime,
        extra: xfl,
        os,
    })
}

/**  
 * TESTS
 */
#[cfg(test)]
mod test {
    use rstest::rstest;

    use crate::{
        header::{read_header, GzipHeader},
        reader::CorniferByteReader,
    };

    #[rstest]
    fn read_header_bails_on_non_gzip_header() {
        let inner: &[u8] = &[5, 6, 7, 0, 1, 2, 3, 4];
        let mut sr = CorniferByteReader::new(Box::new(inner));
        let h = read_header(&mut sr);
        match h {
            Ok(_) => panic!("Return value should have been an Error."),
            Err(e) => assert_eq!(format!("{}", e), "Header is not a GZIP header."),
        };
    }

    #[rstest]
    fn read_header_bails_on_not_deflate() {
        let inner: &[u8] = &[0x1f, 0x8b, 4];
        let mut sr = CorniferByteReader::new(Box::new(inner));
        let h = read_header(&mut sr);
        match h {
            Ok(_) => panic!("Return value should have been an Error."),
            Err(e) => assert_eq!(format!("{}", e), "Compression method must be 8"),
        };
    }

    #[rstest]
    fn read_header_reads_valid_header_minimal() {
        let inner: &[u8] = include_bytes!("../testfiles/helloworld.gz");
        let mut sr = CorniferByteReader::new(Box::new(inner));
        let h = read_header(&mut sr);
        match h {
            Ok(header) => assert_eq!(
                header,
                GzipHeader {
                    comment: None,
                    text: false,
                    name: None,
                    mtime: 0,
                    extra: crate::header::ExtraFlag::Unknown,
                    os: crate::header::OperatingSystem::Unix
                }
            ),
            Err(e) => panic!("{}", e),
        }
    }

    #[rstest]
    fn read_header_reads_valid_text_comment() {
        let inner: &[u8] = include_bytes!("../testfiles/test.gz");
        let mut sr = CorniferByteReader::new(Box::new(inner));
        let h = read_header(&mut sr);
        match h {
            Ok(header) => assert_eq!(
                header,
                GzipHeader {
                    comment: Some("This is a comment".to_string()),
                    text: false,
                    name: Some("filename".to_string()),
                    mtime: 1677648839,
                    extra: crate::header::ExtraFlag::Unknown,
                    os: crate::header::OperatingSystem::Unix
                }
            ),
            Err(e) => panic!("{}", e),
        }
    }

    #[rstest]
    fn read_header_validates_correct_hcrc() {
        let inner: &[u8] = include_bytes!("../testfiles/testCompressThenConcat.txt.gz");
        let mut sr = CorniferByteReader::new(Box::new(inner));
        let h = read_header(&mut sr);
        match h {
            Ok(header) => assert_eq!(
                header,
                GzipHeader {
                    comment: Some("[gzip comment of reasonable length]\n".to_string()),
                    text: true,
                    name: Some("stCompressThenConcat.txt.1".to_string()),
                    mtime: 1274320850,
                    extra: crate::header::ExtraFlag::FastestAlgorithm,
                    os: crate::header::OperatingSystem::Unix
                }
            ),
            Err(e) => panic!("{}", e),
        }
    }

    #[rstest]
    fn read_header_errors_on_incorrect_hcrc() {
        let inner: &[u8] = include_bytes!("../testfiles/testIncorrectHCRC.txt.gz");
        let mut sr = CorniferByteReader::new(Box::new(inner));
        let h = read_header(&mut sr);
        match h {
            Ok(_) => panic!("Should have thrown an error"),
            Err(e) => assert_eq!(
                format!("{}", e),
                "Header CRC is incorrect, expected 0xE8EE but got 0xE7EE"
            ),
        }
    }
}

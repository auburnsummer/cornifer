use std::io::Cursor;

use rusqlite::{blob::ZeroBlob, Connection, DatabaseName};

use crate::{decompress::BlockType, errors::ScryError};

/**
 * Handles writing "checkpoints" (rows in an sqlite table).
 *
 * There are two types of checkpoints. Blocks and ticks.
 *
 * Blocks occur at the beginning of a DEFLATE block. We always emit a checkpoint at every block, and it's guaranteed
 * that all blocks will have a checkpoint.
 *
 * Ticks occur during a DEFLATE block (but never in the middle of a symbol being decoded). These
 * only get emitted if a single deflate block is particularly big and we want random access inside it.
 * 
 * For now, I haven't implemented Ticks yet. It looks like most mainstream GZIP compressors tend to produce
 * blocks fairly regularly, but I'll need to do more research...
 */

fn dist_in_bits(byte1: usize, bit1: u8, byte2: usize, bit2: u8) -> isize {
    let bit2 = bit2 as isize;
    let bit1 = bit1 as isize;
    let byte1 = byte1 as isize;
    let byte2 = byte2 as isize;
    return ((byte2 - byte1) * 8) + (bit2 - bit1);
}

pub struct Checkpointer {
    conn: Connection,
    emit_block_type: BlockType,
    emit_byte: usize,
    emit_bit: u8,
    to_byte: usize,
    current_block_id: i64,
}

fn setup_connection(conn: &Connection) -> Result<(), ScryError> {
    // id: id of the block. not guaranteed to be sequential.
    // from_byte:
    // from_bit  : the byte and bit of the input (i.e. compressed stream) this checkpoint starts at.
    // to_byte   : the byte of the uncompressed output this checkpoint starts at.
    // block_type: "NOCOMPRESSION", "FIXED", or "DYNAMIC"
    // crc32: crc32 of the decompressed data.
    // header_len_bits: length of the header, in bits!!
    // block_len_bits: length of the entire block, including the header, in bits, in the compressed stream.
    // len: length of the entire block, in bytes, in the uncompressed stream.
    // data      : previous bytes of data before this block.
    conn.execute(
        "
    CREATE TABLE HuffmanBlock (
        id  INTEGER PRIMARY KEY AUTOINCREMENT,
        from_byte INTEGER NOT NULL,
        from_bit INTEGER NOT NULL,
        to_byte INTEGER NOT NULL,
        block_type TEXT NOT NULL,
        crc32 TEXT,
        len INTEGER,
        header_len_bits INTEGER,
        block_len_bits INTEGER,
        data BLOB NOT NULL
    )",
        (),
    )?;

    // id
    // from_byte
    // from_bit
    // to_byte  : same as HuffmanBlock
    // block: FK to HuffmanBlock. We need this to get the required huffman trees.
    // data: previous bytes of data before this tick.
    conn.execute(
        "
    CREATE TABLE Tick (
        id  INTEGER PRIMARY KEY AUTOINCREMENT,
        from_byte INTEGER NOT NULL,
        from_bit INTEGER NOT NULL,
        to_byte INTEGER NOT NULL,
        block_id INTEGER NOT NULL,
        data BLOB NOT NULL,
        FOREIGN KEY (block_id) REFERENCES HuffmanBlock (id)
    )",
        (),
    )?;

    Ok(())
}

impl Checkpointer {
    // Initialize a Checkpointer using an sqlite database in file.
    pub fn init(path: String) -> Result<Self, ScryError> {
        let conn = Connection::open(path)?;

        setup_connection(&conn)?;

        Ok(Self {
            conn,
            emit_block_type: BlockType::NoCompression, // gets set on the first BlockHeader state.
            emit_byte: 0,
            emit_bit: 0,
            to_byte: 0,
            current_block_id: 0,
        })
    }

    // Initialize a Checkpointer using an sqlite database in memory.
    // I only expect this to be useful for tests.
    pub fn init_memory() -> Result<Self, ScryError> {
        let conn = Connection::open_in_memory()?;

        setup_connection(&conn)?;

        Ok(Self {
            conn,
            emit_block_type: BlockType::NoCompression, // gets set on the first BlockHeader state.
            emit_byte: 0,
            emit_bit: 0,
            to_byte: 0,
            current_block_id: 0
        })
    }

    pub fn set_block_type(&mut self, block_type: BlockType) {
        self.emit_block_type = block_type;
    }

    // Should be called just where the block starts.
    pub fn on_block_start(&mut self, curr_byte: usize, bit: u8, to_byte: usize) {
        // curr_byte is "where the reader is". if we've already read at least one bit,
        // that byte has been read in its entirety and buffered. hence, the variable curr_byte is
        // already at the _next_ byte.
        // where the block is in the compressed stream.
        self.emit_byte = if bit == 0 { curr_byte } else { curr_byte - 1 };
        self.emit_bit = bit;
        // where the block is in the uncompressed stream.
        self.to_byte = to_byte;
    }

    // Should be called just where the block data starts (after the header)
    pub fn on_block_data_start(
        &mut self,
        curr_byte: usize,
        bit: u8,
        data: Vec<u8>,
    ) -> Result<(), ScryError> {
        let curr_byte = if bit == 0 { curr_byte } else { curr_byte - 1 };
        let block_header_size_bits = dist_in_bits(self.emit_byte, self.emit_bit, curr_byte, bit);

        // block_type string to write to the database.
        let block_type = match self.emit_block_type {
            BlockType::NoCompression => "nocompression",
            BlockType::FixedHuffman => "fixed",
            BlockType::DynamicHuffman => "dynamic",
        };

        self.conn.execute("
            INSERT INTO HuffmanBlock (from_byte, from_bit, to_byte, block_type, header_len_bits, data) VALUES (?1, ?2, ?3, ?4, ?5, ?6)
        ", (self.emit_byte, self.emit_bit, self.to_byte, block_type, block_header_size_bits, ZeroBlob(data.len().try_into().expect("Max size for data will be 32kb, so this should always fit"))))?;

        // Get the row id off the BLOB we just inserted.
        let rowid = self.conn.last_insert_rowid();
        self.current_block_id = rowid;
        // Open the BLOB we just inserted for IO.
        let mut blob =
            self.conn
                .blob_open(DatabaseName::Main, "HuffmanBlock", "data", rowid, false)?;
        let mut file = Cursor::new(data);
        // copy the vector into the SQL blob.
        std::io::copy(&mut file, &mut blob)?;

        Ok(())
    }

    // Should be called just where the block data ends
    pub fn on_block_end(
        &mut self,
        curr_byte: usize,
        bit: u8,
        to_byte: usize,
        crc32: u32
    ) -> Result<(), ScryError> {
        let curr_byte = if bit == 0 { curr_byte } else { curr_byte - 1 };
        // this is the corresponding row that's already been inserted.
        let rowid = self.current_block_id;
        // length of the entire block (compressed)...
        let entire_block_size_bits = dist_in_bits( self.emit_byte, self.emit_bit, curr_byte, bit);
        // length of the block (uncompressed)...
        let uncompressed_block_size = to_byte - self.to_byte;

        // the crc32 as a string
        let formatted_crc = format!("{crc32:x}");

        self.conn.execute("
            UPDATE HuffmanBlock
            SET crc32 = ?1,
                len = ?2,
                block_len_bits = ?3
            WHERE HuffmanBlock.id = ?4
        ", (formatted_crc, uncompressed_block_size, entire_block_size_bits, rowid))?;

        Ok(())
    }
}

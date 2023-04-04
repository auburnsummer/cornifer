use std::io::Cursor;

use rusqlite::{blob::ZeroBlob, Connection, DatabaseName};

use crate::{decompress::BlockType, errors::ScryError};

/**
 * Handles writing "checkpoints" (rows in an sqlite table).
 *
 * There are two types of checkpoints. Blocks and ticks.
 *
 * Blocks occur at the beginning of a DEFLATE block. We always emit a checkpoint at every block.
 *
 * Ticks occur during a DEFLATE block (but never in the middle of a symbol being decoded). These
 * only get emitted if a single deflate block is particularly big and we want random access inside it.
 * 
 * For now, I haven't implemented Ticks yet. It looks like most mainstream GZIP compressors tend to produce
 * blocks fairly regularly, but I'll need to do more research...
 */

pub struct Checkpointer {
    conn: Connection,
    emit_block_type: BlockType,
    emit_byte: usize,
    emit_bit: u8,
    to_byte: usize,
}

fn setup_connection(conn: &Connection) -> Result<(), ScryError> {
    // id: id of the block. not guaranteed to be sequential.
    // from_byte:
    // from_bit  : the byte and bit of the input (i.e. compressed stream) this checkpoint starts at.
    // to_byte   : the byte of the uncompressed output this checkpoint starts at.
    // block_type: "NOCOMPRESSION", "FIXED", or "DYNAMIC"
    // header_len: length of the header, in bits!!. only set if block_type = "DYNAMIC".
    // data      : previous bytes of data before this block.
    conn.execute(
        "
    CREATE TABLE HuffmanBlock (
        id  INTEGER PRIMARY KEY AUTOINCREMENT,
        from_byte INTEGER NOT NULL,
        from_bit INTEGER NOT NULL,
        to_byte INTEGER NOT NULL,
        block_type TEXT NOT NULL,
        header_len INTEGER,
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
        })
    }

    // The Checkpointer can only emit one thing at a time.
    // this sets the block type of the next block to be emitted.
    pub fn set_block_type(&mut self, bt: BlockType) {
        self.emit_block_type = bt;
    }

    // Should be called just where the block starts.
    pub fn set_position(&mut self, curr_byte: usize, bit: u8, to_byte: usize) {
        // curr_byte is "where the reader is". if we've already read at least one bit,
        // that byte has been read in its entirety and buffered. hence, the variable curr_byte is
        // already at the _next_ byte.
        self.emit_byte = if bit == 0 { curr_byte } else { curr_byte - 1 };
        self.emit_bit = bit;
        self.to_byte = to_byte;
    }

    // Should be called just where the block data starts (after the header, etc.)
    pub fn emit_block_checkpoint(
        &self,
        curr_byte: usize,
        bit: u8,
        data: Vec<u8>,
    ) -> Result<(), ScryError> {
        // distance (in bits) is...
        let curr_byte = if bit == 0 { curr_byte } else { curr_byte - 1 } as isize;
        let emit_byte = self.emit_byte as isize;
        let emit_bit = self.emit_bit as isize;
        let curr_bit = bit as isize;
        let distance_in_bits = ((curr_byte - emit_byte) * 8) + (curr_bit - emit_bit);

        let block_type = match self.emit_block_type {
            BlockType::NoCompression => "nocompression",
            BlockType::FixedHuffman => "fixed",
            BlockType::DynamicHuffman => "dynamic",
        };

        self.conn.execute("
            INSERT INTO HuffmanBlock (from_byte, from_bit, to_byte, block_type, header_len, data) VALUES (?1, ?2, ?3, ?4, ?5, ?6)
        ", (emit_byte, emit_bit, self.to_byte, block_type, distance_in_bits, ZeroBlob(data.len().try_into().expect("Max size for data will be 32kb, so this should always fit"))))?;

        // Get the row id off the BLOB we just inserted.
        let rowid = self.conn.last_insert_rowid();
        // Open the BLOB we just inserted for IO.
        let mut blob =
            self.conn
                .blob_open(DatabaseName::Main, "HuffmanBlock", "data", rowid, false)?;
        let mut file = Cursor::new(data);
        // copy the vector into the SQL blob.
        std::io::copy(&mut file, &mut blob)?;

        Ok(())
    }
}

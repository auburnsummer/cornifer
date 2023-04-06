#!/usr/bin/env python3
import argparse
import sqlite3
import zlib
import io
import sys

from struct import *

def bit_aligned_bytes(file, offset_bit):
    shift = offset_bit
    first_byte = int.from_bytes(file.read(1), byteorder="big")
    first_byte_sh = (first_byte >> shift) & 0xFF
    while True:
        next_byte = int.from_bytes(file.read(1), byteorder="big")
        next_byte_sh = (next_byte << (8 - shift)) & 0xFF
        byte = first_byte_sh | next_byte_sh
        yield byte.to_bytes(1, 'little')
        first_byte_sh = (next_byte >> shift) & 0xFF


def main(source_file, checkpoint_file, block_id):
    # Open the SQLite database with Row as row factory
    conn = sqlite3.connect(checkpoint_file)
    conn.row_factory = sqlite3.Row
    c = conn.cursor()

    # Fetch the row with the specified block_id
    c.execute("SELECT * FROM HuffmanBlock WHERE id = ?", (block_id,))
    row = c.fetchone()
    if row is None:
        print(f"No row found with id {block_id}")
        return

    data = row['data']

    vfile = io.BytesIO()
    # write a noncompressed block with the seed data from the row.
    # when the decompressor walks through this block, its 32kb lookback buffer will be populated.
    vfile.write(pack('<B', 0b000))
    vfile.write(pack('<H', len(data)))
    vfile.write(pack('<H', len(data) ^ 0xFFFF))
    vfile.write(data)
    # Open the source file as a binary file
    with open(source_file, "rb") as f:
        # Seek to the start of the block
        f.seek(row["from_byte"])
        aligned_bytes = bit_aligned_bytes(f, row["from_bit"])
        block_len = row["len"]

        bytes_of_this_block = (row["block_len_bits"] // 8) + 1

        # write remaining bytes. it's fine if we're a bit over, because we limit the number of output bytes
        # in the zlib call.
        for byte in (next(aligned_bytes) for _ in range(bytes_of_this_block)):
            vfile.write(byte)

    decompressor = zlib.decompressobj(-15)

    result_data = decompressor.decompress(vfile.getbuffer(), len(data)+block_len)

    result_data_io = io.BytesIO(result_data)
    result_data_io.seek(len(data))
    for byte in result_data_io:
        sys.stdout.buffer.write(byte)

    # Close the database connection
    conn.close()

if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("source_file", help="path to the source file")
    parser.add_argument("checkpoint_file", help="path to the checkpoint file")
    parser.add_argument("block_id", type=int, help="ID of the block to process")
    args = parser.parse_args()
    main(args.source_file, args.checkpoint_file, args.block_id)

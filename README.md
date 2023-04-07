# Cornifer

Cornifer is a script which extracts blocks from a GZIP file and then stores metadata
about those blocks in a SQLite database.

The information in the SQLite database is sufficient to extract individual
blocks in the GZIP file without needing access to the entire file.

Each row in the database contains:

 - The location of the block in the compressed stream;
 - The location of the block in the uncompressed stream;
 - The size of the block in the compressed and uncompressed streams;
 - The header of the block;
 - The CRC32 checksum of the uncompressed block;
 - The preceding 32kb of data in the uncompressed stream before this block.

The [`demo/demo.py`](./demo/demo.py) contains an example of how you might extract
a block with the database.

# Installation

`cargo install cornifer`

# Usage

`cornifer --output-checkpoint ./out.sqlite3 ./file.gz`

Note that Cornifer doesn't write the decompressed file to disk, only the SQLite
database containing the block info. It will tell you the CRC32 of the decompressed
file, so you should check this, e.g.

`gzip -d < file.gz | crc32 /dev/stdin`

# License

AGPLv3
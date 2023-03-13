to make the test file

`perl make.pl`

and

`echo "hello world" | grep` ..

`testCompressThenConcat.txt.gz` was downloaded from the internet.

To determine where the header is in the hexdump, I put a breakpoint in and
ran the test read_header_validates_correct_hcrc in debug. in this case,
the hcrc in the file is 0xE8EE, and it's at byte offset 0xe9 (233)



Then to create the version with the wrong CRC, pipe it through xxd:

cat testCompressThenConcat.txt.gz | xxd > temp

edit the file, then make a new one

cat temp | xxd -r > testIncorrectHCRC.txt.gz
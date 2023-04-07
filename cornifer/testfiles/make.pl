use IO::Compress::Gzip qw(gzip $GzipError);

gzip \"payload" => "./test.gz", 
     Name       => "filename", 
     Comment    => "This is a comment", 
     ExtraField => [ "ab" => "cde"],
     Time => 1677648839,
  or die "Cannot create gzip file: $GzipError" ;
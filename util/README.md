In `formats/` are definitions of the Sonar data formats, as high-level Go code, and simple parsers
for the most important of the formats.  Currently there are definitions for the initial v0.13 and
older format, known here as "old" because there's a "new" format coming.

There is a sample client of the format definition:

* In `ingest-files/` is a program that will attempt to read old-format data from a directory tree.

The ingestor program is a sensible basis for a program that obtains data and stores them in a
database, for example.



# Map processes to applications with sonar map

Map processes to applications:

```
$ sonar map --input-dir /home/user/snap-outputs \
            --str-map-file example-mapping/string_map.txt \
            --re-map-file example-mapping/regex_map.txt
```

The mapping files (`string_map.txt` and `regex_map.txt`) contain a space-separated
(does not matter how many spaces) mapping from process to application.
Example mapping files: https://github.com/uit-no/sonar/tree/master/example-mapping

You are welcome to use your own but encouraged to contribute mappings to our example files.

# Proxy test data

`test-payload.json` is a valid test input for the kafka proxy.  You can send it with

```
curl --data-binary @test-payload.json -H 'Content-Type: application/octet-stream' localhost:8090
```

and if the proxy is run with -D, the file `kafka-proxy.dat` will be equivalent to the input file,
though not exactly the same because some newlines were lost in transit, by design, and the JSON
formatting is a little different.

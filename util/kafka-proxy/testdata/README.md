# Proxy test data

`test-payload.json` is a valid test input for the kafka proxy.  From the parent directory, you can
start a test proxy with

```
./kprox -D testdata/kprox-test.ini &
```

send then send data to it with

```
curl --data-binary @testdata/test-payload.json -H 'Content-Type: application/octet-stream' localhost:8099/kprox-test
```

You should see messages like these:

```
2026/02/24 09:49:42 Message #0 received: my topic my key my client my.user my.password 72
2026/02/24 09:49:42 Dumping message to kafka-proxy.dat, not sending to Kafka
2026/02/24 09:49:42 Message #1 received: my second topic my second key my second client my.user my.password 59
2026/02/24 09:49:42 Dumping message to kafka-proxy.dat, not sending to Kafka
```

Then the file `kafka-proxy.dat` will be equivalent to the input file (though not exactly the same
because some newlines were lost in transit, by design, and the JSON formatting is a little
different).

Don't forget:
```
kill %1
```

package main

import (
	"os"
	"testing"
	"time"
)

func TestParseIncoming(t *testing.T) {
	ch := make(chan Msg, 100)
	bytes, _ := os.ReadFile("testdata/test-payload.json")
	parsePayload(ch, bytes)
	timeout := time.After(time.Second * 5)
	select {
	case msg := <-ch:
		if msg.Topic != "my topic" ||
			msg.Key != "my key" ||
			msg.Client != "my client" ||
			msg.SaslUser != "my.user" ||
			msg.SaslPassword != "my.password" ||
			msg.DataSize != uint64(len(msg.Data)) {
			t.Fatal(msg)
		}
	case <-timeout:
		t.Fatal("Timeout")
	}
	select {
	case msg := <-ch:
		if msg.Topic != "my second topic" ||
			msg.Key != "my second key" ||
			msg.Client != "my second client" ||
			msg.SaslUser != "my.user" ||
			msg.SaslPassword != "my.password" ||
			msg.DataSize != uint64(len(msg.Data)) {
			t.Fatal(msg)
		}
	case <-timeout:
		t.Fatal("Timeout")
	}
}

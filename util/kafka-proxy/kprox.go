// SPDX-License-Identifier: MIT

// Kprox is a very simple Kafka REST proxy, written for Sonar but probably generally useful.
//
// Usage: kprox [options] [inifile-name]
//
// Options:
//
//	-v          Verbose - log non-critical errors also
//	-d          Debug logging (implies -v)
//	-D          http receive only (for debugging; implies -d) with dump to kafka-proxy.dat
//	-U user     User, for use with -D exclusively
//	-P password Password, for use with -D exclusively
//
// Kafka posts data via http to this proxy.  This proxy decodes the traffic and then speaks the
// normal Kafka protocol to the broker, forwarding individual messages to it.  It is not necessary
// for the broker to trust this proxy: the proxy will forward the SASL credentials included in the
// messages with the messages to the broker.
//
// For now, this supports only HTTP, so put it behind a web server to support HTTPS.
//
// Logging is to the syslog by default (without options only critical errors, with -v also
// non-critical errors), and to stderr with -d.
//
// With -D, all validated incoming data are appended to kafka-proxy.dat in the proxy's working
// directory.  With -U and -P, the sasl-user / sasl-password fields must be set in the control
// object and must match the user / password or the message is rejected, not dumped.
//
// # Config file
//
// The config file is on .ini format with these sections:
//
//	[http]
//	endpoint = ...        # default /
//	listen-port = ...     # default 8090
//
// http.endpoint:http.listen-port is the address that the proxy listens on for incoming traffic.
//
//	[kafka]
//	broker-address = ...  # default localhost:9099
//	ca-file = ...         # default none
//	sasl = ...            # default true
//	timeout = ...         # default 1800 seconds
//
// The kafka.broker-address and kafka.ca-file are exactly as for Sonar: they are the broker endpoint
// and the cert required to speak TLS to it, if it's set up that way.
//
// If kafka.sasl is true but the sasl-user/sasl-password are not present in the control object (see
// below) then the message is rejected.
//
// kafka.timeout is how long to hold messages without broker contact before discarding them.
//
// # Protocol
//
// The data received by the proxy shall be in the form of a POST with mime type
// application/octet-stream.  The data represent zero or more individual Kafka messages.  Each
// message consists of two parts, a JSON-format control object followed by an arbitrary data blob.
// Each control object is on a single line and is followed by one newline.  Subsequent to the
// control object and the newline are the data.  The control object can be preceded by an arbitrary
// number of newlines (typically at least one).  The end-of-file can be preceded by an arbitrary
// number of newlines.
//
// The control object has these keys and values:
//
//	topic <string> - the Kafka topic (Sonar: [<prefix>.]<cluster-name>.<data-tag>)
//	key <string> - the Kafka key (Sonar: <node-name>)
//	client <string> - the Kafka client id (Sonar: <cluster-name>/<node-name>)
//	sasl-user <string> - the SASL user name, if the broker requires it (Sonar: the <cluster-name>)
//	sasl-password <string> - the SASL password string, if the broker requires it (Sonar: per cluster)
//	data-size <number> - the size of the data blob following the newline after the control object,
//	                     not including any newlines after that
//
// If sasl-user or sasl-password are present in the control object then both must be present, and a
// connection will be created specially for that pair.  The client ID is forwarded to Kafka as a
// header on the message.
//
// # Limits
//
// It is inevitable that there will be some restrictions in the proxy on the size and volume of
// messages it can handle.  The proxy clients should:
//
//   - never send more than 1GB (2^30 bytes) of payload in any POST
//   - not use more than 1000 sasl-user:sasl-password combinations
//
// These limits are hardcoded in the Proxy for now.  Messages larger than the max payload will be
// rejected with a 400 Too big response.  Once more than 1000 credential pairs have been
// accumulated, additional ones will be rejected and the messages they are associated with will be
// rejected silently.
//
// Probably Kafka messages larger than 1MB will be problematic.
//
// # TODO
//
// There are some denial-of-service risks that are not handled well:
//
//   - Too many concurrently active http request handlers can overwhelm the system and cause OOM even
//     if individual messages are size-limited.  We could have a limit on the number of active
//     handlers.
//   - A flood of too many bogus sasl-user:sasl-password credential pairs can fill up the credential
//     table and cause subsequent legitimate ones to be rejected.  We could partly purge the
//     credential table when it fills up using some simple LRU / timestamping scheme.
package main

import (
	"bytes"
	"context"
	"crypto/tls"
	"crypto/x509"
	"encoding/json"
	"flag"
	"fmt"
	"io"
	"log"
	"log/syslog"
	"net/http"
	"os"
	"strings"
	"time"

	"github.com/lars-t-hansen/ini"
	"github.com/twmb/franz-go/pkg/kgo"
	"github.com/twmb/franz-go/pkg/sasl/plain"
)

//go:generate ./version.bash

const (
	maxCredentials   = 1000
	maxContentLength = int64(1024 * 1024 * 1024)
)

var (
	kafkaBrokerAddress = "localhost:9099"
	kafkaRequireSasl   = true
	kafkaTimeoutSec    = 60 * 30
	kafkaCaFile        = ""
	httpEndpoint       = "/"
	httpListenPort     = 8090
	debug              = flag.Bool("d", false, "Debug logging")
	verbose            = flag.Bool("v", false, "Verbose logging of non-critical errors")
	receiveOnly        = flag.Bool("D", false, "Receive only (for debugging) + dumping")
	debugUser          = flag.String("U", "", "User (for -D only)")
	debugPassword      = flag.String("P", "", "Password (for -D only)")
)

var (
	kafkaCaCert []byte
	syslogger   *syslog.Writer
)

type Control struct {
	Topic        string `json:"topic"`
	Key          string `json:"key"`
	Client       string `json:"client"`
	SaslUser     string `json:"sasl-user,omitempty"`
	SaslPassword string `json:"sasl-password,omitempty"`
	DataSize     uint64 `json:"data-size"`
}

type Msg struct {
	Control
	Data []byte
}

func main() {
	var err error
	syslogger, err = syslog.Dial("", "", syslog.LOG_USER, "kprox")
	if err != nil {
		log.Fatalf("Failing to open syslogger: %v", err)
	}
	defer syslogger.Close()

	flag.Usage = func() {
		fmt.Fprintf(flag.CommandLine.Output(), "kprox Kafka REST proxy version %s\n", version)
		fmt.Fprintf(flag.CommandLine.Output(), "Usage of %s:\n", os.Args[0])
		fmt.Fprintf(flag.CommandLine.Output(), "%s [options] [ini-filename]\n", os.Args[0])
		fmt.Fprintf(flag.CommandLine.Output(), "Options:\n")
		flag.PrintDefaults()
	}
	flag.Parse()
	rest := flag.Args()
	if len(rest) > 0 {
		iniName := rest[0]
		f, err := os.Open(iniName)
		if err != nil {
			log.Fatalf("Ini file %s not found.\nTry -h", iniName)
		}
		iniParser := ini.NewParser()
		kafkaSect := iniParser.AddSection("kafka")
		kBrokerAddr := kafkaSect.AddString("broker-address")
		kRequireSasl := kafkaSect.AddBool("sasl")
		kTimeoutSec := kafkaSect.AddUint64("timeout")
		kCaFile := kafkaSect.AddString("ca-file")
		httpSect := iniParser.AddSection("http")
		hEndpoint := httpSect.AddString("endpoint")
		hListenPort := httpSect.AddUint64("listen-port")
		store, err := iniParser.Parse(f)
		f.Close()
		if err != nil {
			log.Fatalf("Could not parse ini file: %v", err)
		}
		if kBrokerAddr.Present(store) {
			kafkaBrokerAddress = kBrokerAddr.StringVal(store)
		}
		if kRequireSasl.Present(store) {
			kafkaRequireSasl = kRequireSasl.BoolVal(store)
		}
		if kTimeoutSec.Present(store) {
			kafkaTimeoutSec = int(kTimeoutSec.Uint64Val(store))
		}
		kafkaCaFile = kCaFile.StringVal(store)
		if hEndpoint.Present(store) {
			httpEndpoint = hEndpoint.StringVal(store)
		}
		if hListenPort.Present(store) {
			httpListenPort = int(hListenPort.Uint64Val(store))
		}
	}
	if *receiveOnly {
		*debug = true
	}
	if *debug {
		*verbose = true
	}

	if kafkaCaFile != "" {
		var err error
		kafkaCaCert, err = os.ReadFile(kafkaCaFile)
		if err != nil {
			log.Fatalf("Could not read CA cert file %s", kafkaCaFile)
		}
	}
	ch := make(chan Msg, 100)
	if *receiveOnly {
		runDebugDumper(ch)
	} else {
		runKafkaSender(ch)
	}
	runHttpListener(ch)
	report(true, "Kafka HTTP proxy exit: %v", http.ListenAndServe(fmt.Sprintf(":%d", httpListenPort), nil))
	close(ch)
}

func report(emergency bool, format string, args ...any) {
	if emergency {
		syslogger.Emerg(fmt.Sprintf(format, args...))
	} else if *verbose {
		syslogger.Err(fmt.Sprintf(format, args...))
	}
	if *debug {
		log.Printf(format, args...)
	}
}

func runDebugDumper(ch <-chan Msg) {
	go (func() {
		dump, err := os.OpenFile("kafka-proxy.dat", os.O_APPEND|os.O_CREATE|os.O_WRONLY, 0644)
		if err != nil {
			log.Fatal(err)
		}
		defer dump.Close()
		id := uint64(0)
		nl := []byte{'\n'}
		for {
			msg, gotOne := <-ch
			if !gotOne {
				break
			}
			msgId := id
			id++
			if *debug {
				log.Printf(
					"Message #%d received: %s %s %s %s %s %d",
					msgId, msg.Topic, msg.Key, msg.Client, msg.SaslUser, msg.SaslPassword, msg.DataSize,
				)
				log.Printf("Dumping message to kafka-proxy.dat, not sending to Kafka")
			}
			bs, _ := json.Marshal(msg.Control)
			_, _ = dump.Write(bs)
			_, _ = dump.Write(nl)
			_, _ = dump.Write(msg.Data)
			_, _ = dump.Write(nl)
		}
	})()
}

func checkCredentials(c Control) bool {
	// If no -U then accept every user
	if *debugUser != "" && c.SaslUser != *debugUser {
		return false
	}
	// If no -P then accept every password
	if *debugPassword != "" && c.SaslPassword != *debugPassword {
		return false
	}
	return true
}

func runKafkaSender(ch <-chan Msg) {
	go (func() {
		clients := make(map[string]*kgo.Client)
		id := uint64(0)
		for {
			msg, gotOne := <-ch
			if !gotOne {
				break
			}
			msgId := id
			id++
			if *debug {
				log.Printf(
					"Message #%d received: %s %s %s %s %s %d",
					msgId, msg.Topic, msg.Key, msg.Client, msg.SaslUser, msg.SaslPassword, msg.DataSize,
				)
			}
			if kafkaRequireSasl && msg.SaslUser == "" && msg.SaslPassword == "" {
				if *debug {
					log.Printf("Rejecting message b/c no Sasl credentials")
				}
				continue
			}
			// Note we can't easily use client as the client id without creating one kgo.Client for
			// each node that sends us data.  So attach client as a header to the record, to
			// indicate the originating client.
			record := &kgo.Record{
				Key:   []byte(msg.Key),
				Topic: msg.Topic,
				Value: msg.Data,
				Headers: []kgo.RecordHeader{
					kgo.RecordHeader{Key: "Originator", Value: []byte(msg.Client)},
					kgo.RecordHeader{Key: "Id", Value: []byte(fmt.Sprint(msgId))},
				},
			}
			clientId := msg.SaslUser + "|" + msg.SaslPassword
			cl := clients[clientId]
			if cl == nil {
				if len(clients) == maxCredentials {
					if *debug {
						log.Printf("Rejecting message b/c too many credentials")
					}
					continue
				}
				var err error
				opts := []kgo.Opt{
					kgo.ClientID("kprox-" + version),
					kgo.SeedBrokers(kafkaBrokerAddress),
					kgo.AllowAutoTopicCreation(),
				}
				if msg.SaslUser != "" || msg.SaslPassword != "" {
					opts = append(opts, kgo.SASL(plain.Auth{
						User: msg.SaslUser,
						Pass: msg.SaslPassword,
					}.AsMechanism()))
				}
				if kafkaCaCert != nil {
					caCertPool := x509.NewCertPool()
					caCertPool.AppendCertsFromPEM(kafkaCaCert)
					tlsConfig := &tls.Config{RootCAs: caCertPool}
					opts = append(opts, kgo.DialTLSConfig(tlsConfig))
				}
				cl, err = kgo.NewClient(opts...)
				if err != nil {
					report(true, "Failed to create client: %v", err)
					continue
				}
				clients[clientId] = cl
			}
			// Fire and forget, mostly.  The Kafka module takes care of connections and servers that
			// go down and come up, and will hold records until they are sent or time out.
			ctx, cancel := context.WithDeadline(
				context.Background(),
				time.Now().Add(time.Second*time.Duration(kafkaTimeoutSec)),
			)
			cl.Produce(ctx, record, func(rec *kgo.Record, err error) {
				cancel()
				id := "???"
				for _, h := range rec.Headers {
					if h.Key == "Id" {
						id = string(h.Value)
					}
				}
				if err != nil {
					report(true, "Error produced for id=%s: %v", id, err)
				} else {
					if *debug {
						log.Printf("Message delivered id=%s", id)
					}
				}
			})
		}
		// Give things time to settle before exiting
		time.Sleep(time.Second * 10)
		for _, cl := range clients {
			cl.Close()
		}
	})()
}

func runHttpListener(ch chan<- Msg) {
	http.HandleFunc(httpEndpoint, func(w http.ResponseWriter, r *http.Request) {
		if r.Method != "POST" {
			report(false, "Bad method %s", r.Method)
			w.WriteHeader(403)
			fmt.Fprintf(w, "Bad method")
			return
		}
		ct, ok := r.Header["Content-Type"]
		if !ok || ct[0] != "application/octet-stream" {
			if !ok {
				report(false, "No content-type")
			} else {
				report(false, "Bad content-type %s", ct)
			}
			w.WriteHeader(400)
			fmt.Fprintf(w, "Bad content-type")
			return
		}

		if r.ContentLength > maxContentLength {
			w.WriteHeader(400)
			fmt.Fprintf(w, "Too big")
			return
		}

		payload := make([]byte, r.ContentLength)
		haveRead := 0
		for haveRead < int(r.ContentLength) {
			n, err := r.Body.Read(payload[haveRead:])
			haveRead += n
			if err != nil {
				if err == io.EOF && haveRead == int(r.ContentLength) {
					break
				}
				report(false, "Failed to read content")
				w.WriteHeader(400)
				fmt.Fprintf(w, "Bad content")
				return
			}
		}

		statusCode, message := parsePayload(ch, payload)
		w.WriteHeader(statusCode)
		fmt.Fprint(w, message)
	})
}

func parsePayload(ch chan<- Msg, payload []byte) (int, string) {
	ix := 0
	for {
		// Extract the next control object.  Skip any preceding newlines, then look for the
		// object on a line by itself.
		for ix < len(payload) && payload[ix] == '\n' {
			ix++
		}
		if ix == len(payload) {
			break
		}
		loc := bytes.IndexByte(payload[ix:], '\n')
		if loc == -1 {
			report(false, "Trailing junk in message")
			return 400, "Trailing junk"
		}
		var c Control
		controlObject := payload[ix : ix+loc]
		err := json.Unmarshal(controlObject, &c)
		if err != nil {
			report(false, "Could not decode a control object: %v\n%s", err, string(controlObject))
			return 400, "Malformed control object"
		}

		// Consume the control object and single newline we know is there, because we found it
		ix += loc + 1

		// Extract the data, and forward the control object and data to the Kafka thread.
		endIx := ix + int(c.DataSize)
		if endIx > len(payload) {
			report(false, "Out of bounds data length for %s", string(controlObject))
			return 400, "Out of bounds data length"
		}

		// Launder the control fields.
		c.SaslUser = strings.TrimSpace(c.SaslUser)
		c.SaslPassword = strings.TrimSpace(c.SaslPassword)
		c.Topic = strings.TrimSpace(c.Topic)
		c.Key = strings.TrimSpace(c.Key)
		c.Client = strings.TrimSpace(c.Client)

		// In -D mode, check credentials.  Logically this test belongs in runDebugDumper, but by
		// having it here we can return a sensible response code and thus the client can test the
		// transmission of the credentials.
		if *receiveOnly {
			if !checkCredentials(c) {
				report(false, "Bad credentials")
				return 401, "Bad credentials"
			}
		}

		// Produce it.
		ch <- Msg{Control: c, Data: payload[ix:endIx]}

		// Consume the data
		ix = endIx
	}
	return 200, "Ok"
}

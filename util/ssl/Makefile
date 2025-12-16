# Note the default expiration time for the artifacts produced here is 1 year, the longest
# allowed by best practices.

HOSTNAME ?= $(shell hostname)
SUBJECT_ALT_NAME ?= "DNS:$(HOSTNAME),DNS:localhost,IP:127.0.0.1"
DAYS ?= 365

.PHONY: build default all clean

default:
	@echo "Use an explicit target"

all: server-key.pem server-cert-signed.crt sonar-ca.crt sonar-kafka-keystore.pem

build: clean all

clean:
	rm -f *.pem *.crt *.csr *.key *.srl

# Server private/public keys
server-key.pem server-cert.pem: Makefile
	openssl req -x509 -newkey rsa:4096 -keyout server-key.pem -out server-cert.pem \
	-sha256 \
	-days $(DAYS) \
	-nodes \
	-subj "/CN=$(USER).$(HOSTNAME)" \
	-addext "subjectAltName=$(SUBJECT_ALT_NAME)"

# CA private key.
sonar-ca-key.pem: Makefile
	openssl genrsa -out sonar-ca-key.pem 2048

# CA self-signed cert
# TODO: Totally not obvious why this would need to know about SAN.
sonar-ca.crt: sonar-ca-key.pem Makefile
	openssl req -new -x509 -key sonar-ca-key.pem -out sonar-ca.crt \
	-days $(DAYS) \
	-subj "/CN=$(USER).$(HOSTNAME) CA" \
	-addext "subjectAltName=$(SUBJECT_ALT_NAME)"

# Server certificate signing request
server-key.csr: server-key.pem Makefile
	openssl req -new -key server-key.pem -out server-key.csr \
	-subj "/CN=$(USER).$(HOSTNAME)" \
	-addext "subjectAltName=$(SUBJECT_ALT_NAME)"

# Server signed certificate
server-cert-signed.crt: server-key.csr sonar-ca.crt sonar-ca-key.pem Makefile
	openssl x509 -req \
	-in server-key.csr \
	-days $(DAYS) \
	-CA sonar-ca.crt -CAkey sonar-ca-key.pem -CAcreateserial \
	-out server-cert-signed.crt \
	-copy_extensions copyall

# The keystore is used by the Kafka server config for ssl.keystore.location, it's just a PEM
# containing the server's private key and signed public key, catenated.
sonar-kafka-keystore.pem: server-key.pem server-cert-signed.crt Makefile
	cat server-key.pem server-cert-signed.crt > sonar-kafka-keystore.pem

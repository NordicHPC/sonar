package main

import (
	"bufio"
	"fmt"
	"io"
	"strings"
)

// Authentication abstraction.
//
// A password file has a sequence of lines, each with a username:password syntax.  Blanks at the
// beginning and end of line as well as on the end of the username and beginning of the password are
// stripped, and empty lines are ignored.  Empty username or password (after stripping) are
// explicitly disallowed and clients can depend on that.  The file can be read with
// NewAuthenticator() to produce an Authenticator object that can be used to authenticate
// credentials.  The Authenticator is thread-safe.
//
// MT: Immutable after creation
type Authenticator struct {
	identities map[string]string
}

func NewAuthenticator(pwfile io.Reader) (*Authenticator, error) {
	scanner := bufio.NewScanner(pwfile)
	identities := make(map[string]string)
	for scanner.Scan() {
		l := strings.TrimSpace(scanner.Text())
		if l == "" {
			continue
		}
		name, pass, found := strings.Cut(l, ":")
		if !found {
			return nil, fmt.Errorf("Bad line in password file: %s", l)
		}
		name = strings.TrimSpace(name)
		pass = strings.TrimSpace(pass)
		if name == "" || pass == "" {
			return nil, fmt.Errorf("Bad line in password file: %s", l)
		}
		identities[name] = pass
	}
	return &Authenticator{identities: identities}, nil
}

func (a *Authenticator) Authenticate(user, pass string) bool {
	probe, found := a.identities[user]
	return found && probe == pass
}

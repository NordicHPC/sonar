use crate::output;

// Parse a nodelist and render it into an output object as an array of strings.

pub fn parse_and_render(xs: &str) -> Result<output::Array, String> {
    let mut a = output::Array::new();
    for v in parse(xs)? {
        a.push_s(v);
    }
    Ok(a)
}

// Split a slurm nodelist into its elements.
//
// This must parse the list b/c the commas in node sets can be confused with the commas between node
// ranges - we can't just use split().
//
// The grammar is:
//
//   nodelist  ::= element ("," element)*
//   element   ::= fragment+
//   fragment  ::= literal | range
//   literal   ::= <longest nonempty string of characters not containing "[" or ",">
//   range     ::= "[" range-elt ("," range-elt)* "]"
//   range-elt ::= number | number "-" number
//   number    ::= <nonempty string of 0..9, to be interpreted as decimal>
//
// There are no spaces.

pub fn parse(xs: &str) -> Result<Vec<String>, String> {
    let mut p = NodelistParser {
        s: xs.as_bytes(),
        i: 0,
    };
    let mut a = vec![];
    a.push(p.element()?);
    while p.eat(b',') {
        a.push(p.element()?);
    }
    if !p.at_end() {
        return Err("Trailing junk".to_string());
    }
    Ok(a)
}

struct NodelistParser<'a> {
    s: &'a [u8],
    i: usize,
}

impl NodelistParser<'_> {
    fn element(&mut self) -> Result<String, String> {
        let start = self.i;
        if !self.fragment()? {
            return Err("Empty input".to_string());
        }
        while self.fragment()? {
            // nothing
        }
        match std::str::from_utf8(&self.s[start..self.i]) {
            Ok(s) => Ok(s.to_string()),
            Err(_) => {
                panic!("Should not happen")
            }
        }
    }

    fn fragment(&mut self) -> Result<bool, String> {
        Ok(self.maybe_range()? || self.maybe_literal()?)
    }

    fn maybe_literal(&mut self) -> Result<bool, String> {
        let start = self.i;
        while !(self.at_end() || self.peek(b'[') || self.peek(b',')) {
            self.i += 1
        }
        Ok(start < self.i)
    }

    fn maybe_range(&mut self) -> Result<bool, String> {
        if !self.eat(b'[') {
            return Ok(false);
        }
        self.range_elt()?;
        while self.eat(b',') {
            self.range_elt()?;
        }
        if self.eat(b']') {
            Ok(true)
        } else {
            Err("Missing ']'".to_string())
        }
    }

    fn range_elt(&mut self) -> Result<(), String> {
        self.number()?;
        if self.eat(b'-') {
            self.number()?;
        }
        Ok(())
    }

    fn number(&mut self) -> Result<(), String> {
        if !self.eat_digit() {
            return Err("Number expected".to_string());
        }
        while self.eat_digit() {}
        Ok(())
    }

    fn eat_digit(&mut self) -> bool {
        if !self.at_end() {
            let c = self.s[self.i];
            if c >= b'0' && c <= b'9' {
                self.i += 1;
                return true;
            }
        }
        false
    }

    fn eat(&mut self, c: u8) -> bool {
        if self.peek(c) {
            self.i += 1;
            return true;
        }
        false
    }

    fn peek(&mut self, c: u8) -> bool {
        !self.at_end() && self.s[self.i] == c
    }

    fn at_end(&mut self) -> bool {
        self.i == self.s.len()
    }
}

#[test]
pub fn test_parser() {
    match parse("c1-[5-6,8-9,12-20,25,28],c1-[21,23],bigmem-2,c1-29") {
        Ok(xs) => {
            assert!(xs[0] == "c1-[5-6,8-9,12-20,25,28]");
            assert!(xs[1] == "c1-[21,23]");
            assert!(xs[2] == "bigmem-2");
            assert!(xs[3] == "c1-29");
        }
        Err(_) => {
            assert!(false, "Parsing failed")
        }
    }

    assert!(parse("").is_err());
    assert!(parse("zappa,").is_err());
    assert!(parse("zappa,,").is_err());
    assert!(parse(",zappa").is_err());
    assert!(parse("zappa[").is_err());
    assert!(parse("zappa[1").is_err());
    assert!(parse("zappa[1,").is_err());
    assert!(parse("zappa[1,]").is_err());
    assert!(parse("zappa[1-").is_err());
    assert!(parse("zappa[1-]").is_err());
    assert!(parse("zappa[1-3").is_err());
    assert!(parse("zappa[1-3,").is_err());
    assert!(parse("zappa[1-3,]").is_err());
    assert!(parse("zappa[x").is_err());
    assert!(parse("zappa[-").is_err());
}

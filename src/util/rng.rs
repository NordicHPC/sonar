// Generate low-quality but fast randomish u32 numbers.

#[allow(dead_code)]
pub struct Rng {
    state: u32, // nonzero
}

#[allow(dead_code)]
impl Rng {
    pub fn new() -> Rng {
        Rng {
            state: crate::posix::time::unix_now() as u32,
        }
    }

    // https://en.wikipedia.org/wiki/Xorshift, this supposedly has period 2^32-1 but is not "very
    // random".
    pub fn next(&mut self) -> u32 {
        let mut x = self.state;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.state = x;
        x
    }
}

#[test]
pub fn rng_test() {
    let mut r = Rng::new();
    let a = r.next();
    let b = r.next();
    let c = r.next();
    let d = r.next();
    // It's completely unlikely that they're all equal, so that would indicate some kind of bug.
    assert!(!(a == b && b == c && c == d));
}

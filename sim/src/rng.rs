//! A small seeded generator. All randomness in the project lives in the
//! simulator; the rulebook and the contract never use any.

/// xoshiro256++, seeded through splitmix64.
#[derive(Clone)]
pub struct Rng {
    s: [u64; 4],
}

fn splitmix(state: &mut u64) -> u64 {
    *state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut z = *state;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

/// FNV-1a, to give each scenario its own stream from the same seed list.
pub fn hash(text: &str) -> u64 {
    let mut h = 0xCBF2_9CE4_8422_2325u64;
    for byte in text.bytes() {
        h ^= byte as u64;
        h = h.wrapping_mul(0x0000_0100_0000_01B3);
    }
    h
}

impl Rng {
    pub fn new(seed: u64, stream: u64) -> Self {
        let mut state = seed ^ stream.rotate_left(32);
        Rng { s: [splitmix(&mut state), splitmix(&mut state), splitmix(&mut state), splitmix(&mut state)] }
    }

    pub fn next(&mut self) -> u64 {
        let result = self.s[0].wrapping_add(self.s[3]).rotate_left(23).wrapping_add(self.s[0]);
        let t = self.s[1] << 17;
        self.s[2] ^= self.s[0];
        self.s[3] ^= self.s[1];
        self.s[1] ^= self.s[2];
        self.s[0] ^= self.s[3];
        self.s[2] ^= t;
        self.s[3] = self.s[3].rotate_left(45);
        result
    }

    /// Uniform in [0, 1).
    pub fn unit(&mut self) -> f64 {
        (self.next() >> 11) as f64 / (1u64 << 53) as f64
    }

    pub fn below(&mut self, n: u64) -> u64 {
        (self.unit() * n as f64) as u64
    }

    pub fn chance(&mut self, p: f64) -> bool {
        self.unit() < p
    }

    /// Exponential waiting time with the given rate.
    pub fn exponential(&mut self, rate: f64) -> f64 {
        -(1.0 - self.unit()).ln() / rate
    }

    pub fn normal(&mut self) -> f64 {
        let (u, v) = (1.0 - self.unit(), self.unit());
        (-2.0 * u.ln()).sqrt() * (std::f64::consts::TAU * v).cos()
    }

    /// Log-normal with the given median and log-scale spread.
    pub fn lognormal(&mut self, median: f64, sigma: f64) -> f64 {
        median * (sigma * self.normal()).exp()
    }
}

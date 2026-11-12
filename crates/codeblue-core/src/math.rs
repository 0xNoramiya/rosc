//! Integer helpers. The rulebook never touches floating point.

/// `floor(a * b / c)` without overflowing when `a` is a full token amount.
pub fn mul_div(a: u128, b: u64, c: u64) -> u128 {
    assert!(c != 0, "division by zero");
    let (b, c) = (b as u128, c as u128);
    let q = a / c;
    let r = a % c;
    // a*b/c = q*b + r*b/c exactly, and r*b < 2^64 * 2^64.
    q * b + r * b / c
}

#[cfg(test)]
mod tests {
    use super::mul_div;

    #[test]
    fn matches_plain_arithmetic_when_small() {
        for a in [0u128, 1, 7, 9_999, 10_000, 123_456_789] {
            for b in [0u64, 1, 50, 9_999, 144_000] {
                for c in [1u64, 3, 100, 10_000] {
                    assert_eq!(mul_div(a, b, c), a * b as u128 / c as u128);
                }
            }
        }
    }

    #[test]
    fn survives_amounts_near_the_whole_supply() {
        let supply = 2_500_000_000u128 * 1_000_000_000_000_000_000;
        assert_eq!(mul_div(supply, 10_000, 10_000), supply);
        assert_eq!(mul_div(supply, 12_000_000, 1_000_000), supply * 12);
    }
}

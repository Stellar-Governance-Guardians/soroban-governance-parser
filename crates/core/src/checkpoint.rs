//! Pure replica of the pinned Script3 votes checkpoint model.
//!
//! Pinned source: `github.com/script3/soroban-governor` @
//! `a2ac6de81055be5bd13e31f922c9546309bfdb8a`,
//! `contracts/votes/src/checkpoints.rs`.
//!
//! A checkpoint packs `(sequence: u32, amount: i128)` into a `u128` as
//! `sequence` in the top 32 bits and `amount` in the low 96 bits
//! (`0x{sequence}{amount}`), so the vector stays sorted by sequence and a
//! ledger lookup is a binary search over packed values.
//!
//! # What `upper_lookup` actually returns
//!
//! The upstream doc comment says "greater than or equal to the given
//! sequence", but the implementation (and upstream's own test vectors)
//! return the amount of the checkpoint with the **largest sequence ≤ query** —
//! a floor lookup — and `0` when no such checkpoint exists or the history is
//! empty. This replica reproduces the *behavior*, including that edge, and the
//! unit tests below are upstream's vectors transcribed verbatim.

/// Upstream `contracts/votes/src/constants.rs::MAX_CHECKPOINT_AGE_LEDGERS`
/// (`15 * ONE_DAY_LEDGERS`, 2880 ledgers/day on Stellar).
pub const MAX_CHECKPOINT_AGE_LEDGERS: u32 = 15 * 2_880;

/// The amount field occupies 96 bits; anything wider cannot be represented.
pub const MAX_CHECKPOINT_AMOUNT: i128 = (1i128 << 96) - 1;

/// Errors the upstream contract traps on (`TokenVotesError`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckpointError {
    /// Amount ≥ 2^96 or negative. Upstream: `InvalidCheckpointError` (#102).
    InvalidAmount(i128),
}

impl core::fmt::Display for CheckpointError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::InvalidAmount(a) => write!(
                f,
                "checkpoint amount {a} does not fit in the packed u96 field"
            ),
        }
    }
}

impl std::error::Error for CheckpointError {}

/// Pack `(sequence, amount)` the way upstream `from_checkpoint_data` does.
///
/// Fail-closed: an amount outside `0..=u96::MAX` is an `Err`, exactly where
/// upstream panics with `InvalidCheckpointError` — never a silent truncation.
pub fn pack(sequence: u32, amount: i128) -> Result<u128, CheckpointError> {
    if !(0..=MAX_CHECKPOINT_AMOUNT).contains(&amount) {
        return Err(CheckpointError::InvalidAmount(amount));
    }
    Ok((u128::from(sequence) << 96) | amount.cast_unsigned())
}

/// Unpack a stored checkpoint into `(sequence, amount)`.
///
/// The amount field is unsigned by construction, so this cannot fail.
pub fn unpack(packed: u128) -> (u32, i128) {
    let sequence = (packed >> 96) as u32;
    // Low 96 bits are the (non-negative) amount; the top 32 are the sequence.
    let amount = packed & ((1u128 << 96) - 1);
    // The low 96 bits hold a non-negative amount by construction.
    (sequence, amount.cast_signed())
}

/// Floor lookup over a sorted checkpoint vector: the amount of the checkpoint
/// with the largest `sequence <= query`, or `0` if none exists (including an
/// empty history).
///
/// Mirrors upstream `upper_lookup` exactly, including searching with the
/// maximum packed amount for the query sequence so that an exact-sequence hit
/// resolves to that checkpoint.
pub fn upper_lookup(checkpoints: &[u128], sequence: u32) -> i128 {
    if checkpoints.is_empty() {
        return 0;
    }
    let Ok(needle) = pack(sequence, MAX_CHECKPOINT_AMOUNT) else {
        // sequence is always a valid u32 so pack cannot fail here; fail closed
        // rather than index on an uninitialized needle.
        return 0;
    };
    match checkpoints.binary_search(&needle) {
        Ok(index) => unpack(checkpoints[index]).1,
        Err(0) => 0,
        Err(index) => unpack(checkpoints[index - 1]).1,
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    fn cp(sequence: u32, amount: i128) -> u128 {
        pack(sequence, amount).expect("test amounts fit u96")
    }

    #[test]
    fn upstream_lookup_vectors_transcribed_verbatim() {
        // github.com/script3/soroban-governor @ a2ac6de… checkpoints.rs
        // `test_upper_lookup`, expectations copied from upstream's own test.
        let checkpoints = [cp(123, 8_293_480), cp(124, 1_234_567), cp(130, 9_876_543)];
        assert_eq!(
            upper_lookup(&checkpoints, 122),
            0,
            "before first checkpoint"
        );
        assert_eq!(upper_lookup(&checkpoints, 123), 8_293_480, "exact hit");
        assert_eq!(upper_lookup(&checkpoints, 124), 1_234_567, "exact hit");
        assert_eq!(
            upper_lookup(&checkpoints, 129),
            1_234_567,
            "between checkpoints"
        );
        assert_eq!(upper_lookup(&checkpoints, 199), 9_876_543, "after last");
    }

    #[test]
    fn empty_history_is_zero_not_an_error() {
        // Upstream `test_upper_lookup_empty`.
        assert_eq!(upper_lookup(&[], 0), 0);
        assert_eq!(upper_lookup(&[], u32::MAX), 0);
    }

    #[test]
    fn pack_unpack_round_trips() {
        for (seq, amt) in [(0u32, 0i128), (1, 1), (5_083_633, 15_500_000_000_000)] {
            let packed = cp(seq, amt);
            assert_eq!(unpack(packed), (seq, amt));
        }
        assert_eq!(
            unpack(cp(u32::MAX, MAX_CHECKPOINT_AMOUNT)),
            (u32::MAX, MAX_CHECKPOINT_AMOUNT)
        );
    }

    #[test]
    fn amount_outside_u96_fails_closed() {
        // Upstream traps with InvalidCheckpointError (#102) for both cases.
        assert_eq!(
            pack(123, MAX_CHECKPOINT_AMOUNT + 1),
            Err(CheckpointError::InvalidAmount(MAX_CHECKPOINT_AMOUNT + 1))
        );
        assert_eq!(pack(123, -1), Err(CheckpointError::InvalidAmount(-1)));
        assert_eq!(
            pack(123, i128::MIN),
            Err(CheckpointError::InvalidAmount(i128::MIN))
        );
        assert!(pack(123, MAX_CHECKPOINT_AMOUNT).is_ok());
    }

    #[test]
    fn sorted_order_is_by_sequence_then_amount() {
        // The vector stays sorted because the sequence occupies the high bits.
        let mut v = vec![cp(5, 10), cp(2, 99), cp(5, 1), cp(9, 0)];
        v.sort_unstable();
        assert_eq!(
            v.iter().map(|&p| unpack(p)).collect::<Vec<_>>(),
            vec![(2, 99), (5, 1), (5, 10), (9, 0)]
        );
        // A same-sequence query resolves to the checkpoint at that sequence
        // (search key uses the max amount, so binary_search lands after them).
        assert_eq!(upper_lookup(&v, 5), 10);
        assert_eq!(upper_lookup(&v, 4), 99);
    }
}

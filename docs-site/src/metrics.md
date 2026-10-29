# Metrics

> **Status: planned.** No metric below is served today: the indexer has no
> GraphQL API (I4). The `governance-v1` SDL already exposes `votesCast`,
> `proposalsEligible`, `participationRatio` and `delegatedPower` on
> `DelegateMetrics`; the remaining definitions are the published contract the
> API must implement.

Delegate behaviour is reported as **raw counts plus a published formula**. There
is no composite score and no ranking. Every ratio below names its numerator and
its denominator, and both inputs are returned next to it so a reader can
recompute the number.

## Eligible proposals

**Definition.** Proposals that were open for voting while the delegate held
voting power greater than zero at the proposal's snapshot ledger.

- Numerator/denominator: this is a **count**, not a ratio. It is the
  denominator of participation.
- Snapshot rule: power is read at the proposal's voting-start ledger from
  checkpointed power, not at the delegate's current balance.
- Exclusions: proposals whose voting window closed before the indexer's cursor
  started (unreadable retention range) are **not** counted, and the open gap is
  reported on the health surface.

## Participation

**Definition.** Of the proposals the delegate was eligible for, the share on
which they cast any vote.

```text
participation = votesCast / proposalsEligible
```

- Numerator: `votesCast` — governance votes cast by the delegate that the
  indexer decoded, one per proposal.
- Denominator: `proposalsEligible` — as defined above.
- `proposalsEligible = 0` ⇒ `participationRatio` is `null`, not `0`. A zero
  denominator is not "no participation".

## Outcome alignment

**Definition.** Of the proposals the delegate voted on, the share where their
vote side matches the proposal's final outcome.

```text
outcome_alignment = votesWithWinningOutcome / votesCast
```

- Numerator: votes whose side (for / against / abstain) equals the recorded
  final outcome.
- Denominator: `votesCast`.
- Ties and cancelled proposals: excluded from both numerator and denominator,
  and counted in a separate `excludedProposals` figure.
- This is a descriptive ratio about past votes. It is not a prediction and not a
  quality score.

## Power-majority alignment

**Definition.** Of the proposals the delegate voted on, the share where their
side matches the side that held the majority of voting power.

```text
power_majority_alignment = votesWithPowerMajority / votesCast
```

- Numerator: votes whose side equals the side with the greater total voting
  power at close.
- Denominator: `votesCast`.
- Excluded: proposals with no votes, exact power ties, and proposals whose
  tally could not be reconstructed (recorded as `unverified`).

## Abstain rate

**Definition.** Of the delegate's votes, the share that abstained.

```text
abstain_rate = abstainVotes / votesCast
```

- Numerator: votes with side `abstain`.
- Denominator: `votesCast`.

## Median vote latency

**Definition.** The median number of ledgers between a proposal becoming
eligible and the delegate's vote, over the proposals they voted on.

```text
latency_i = voteLedger_i - votingStartLedger_i
median_vote_latency = median(latency_i)
```

- Unit: ledgers, reported as a median. The count of samples (`votesCast`) is
  returned with it; a median over a small sample is reported as such, not
  hidden.
- Votes cast in the same ledger as voting start have latency `0`.
- No votes ⇒ `null`, not `0`.

## Data rules

- Numbers are rendered exactly as the API returned them. Nothing is rounded away
  and nothing is combined into a single score.
- Rows produced by fixture replay carry `source='fixture-replay'`; live rows
  carry `source='live'`. No API response mixes them silently.
- If a denominator is zero, the ratio is `null`. A missing input is never
  rendered as zero.

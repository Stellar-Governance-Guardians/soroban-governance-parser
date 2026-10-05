//! Testnet fixture generator contract (charter rule 1: test data is either
//! captured from live testnet or produced by scripts/seed-testnet.*).
//!
//! Emits governance-shaped events whose topic layout mirrors the VERIFIED
//! Script3 soroban-governor shapes recorded in docs/adapters/script3.md:
//! - ["proposal_created", id: u32, creator: Address] data (title, threshold)
//! - ["vote_cast", proposal_id: u32, voter: Address] data (support: u32, amount: u128)
//!
//! This is a fixture, not a governor: no quorum, no timelock, no state machine.

#![no_std]

use soroban_sdk::{
    contract, contractimpl, contracttype, Address, Env, String, Symbol,
};

#[contracttype]
#[derive(Clone)]
pub enum DataKey {
    Proposal(u32),
    Count,
}

#[contracttype]
#[derive(Clone)]
pub struct ProposalRecord {
    pub creator: Address,
    pub title: String,
    pub threshold: u128,
}

#[contract]
pub struct FixtureGovernor;

#[contractimpl]
impl FixtureGovernor {
    /// Create a proposal. Emits `proposal_created` (Script3-shaped topics).
    pub fn propose(env: Env, creator: Address, title: String, threshold: u128) -> u32 {
        creator.require_auth();
        let id: u32 = env
            .storage()
            .instance()
            .get::<DataKey, u32>(&DataKey::Count)
            .unwrap_or(0)
            .wrapping_add(1);
        env.storage().instance().set(&DataKey::Count, &id);
        env.storage()
            .persistent()
            .set(&DataKey::Proposal(id), &ProposalRecord {
                creator: creator.clone(),
                title: title.clone(),
                threshold,
            });
        env.events()
            .publish((Symbol::new(&env, "proposal_created"), id, creator), (title, threshold));
        id
    }

    /// Cast a vote. Emits `vote_cast` (Script3-shaped topics).
    /// support: 0=Against, 1=For, 2=Abstain (same encoding as Script3 governor).
    pub fn vote(env: Env, voter: Address, proposal_id: u32, support: u32) {
        voter.require_auth();
        let amount: u128 = 1_000_000;
        env.events()
            .publish((Symbol::new(&env, "vote_cast"), proposal_id, voter), (support, amount));
    }

    /// Read a stored proposal.
    pub fn get_proposal(env: Env, proposal_id: u32) -> Option<ProposalRecord> {
        env.storage().persistent().get(&DataKey::Proposal(proposal_id))
    }
}

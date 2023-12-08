use crate::{proposal::Proposal, transaction::Transaction, types::Address, validators::Validator};
use cosmwasm_std::Uint128;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct DigiBlock {
    pub index: u64,
    pub timestamp: u64,
    pub merkle_root: String,
    pub transactions: Vec<Transaction>,
    pub proposals: Vec<Proposal>,
    pub previous_hash: String,
    pub sign: String,
    pub proposed_by: Address,
    pub hash: String,
}

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

impl DigiBlock {
    pub fn is_valid(self) -> bool {
        // no of tx should be <= 100
        true
    }
    pub fn get_transactions(self) -> Vec<Transaction> {
        self.transactions
    }
    pub fn get_proposals(self) -> Vec<Proposal> {
        self.proposals
    }

    pub fn create_block(
        validator: Validator,
        timestamp: u64,
        block_number: u64,
        previous_hash: String,
        transactions: Vec<Transaction>,
        proposals: Vec<Proposal>,
    ) -> DigiBlock {
        let mut block = DigiBlock {
            index: block_number,
            timestamp,
            merkle_root: "".to_string(),
            transactions,
            previous_hash,
            proposals,
            hash: "".to_string(),
            sign: "".to_string(),
            proposed_by: validator.acccount.address,
        };
        block.merkle_root = block.clone().calculate_merkle_root();
        block.hash = block.clone().get_block_hash();
        block
    }

    fn get_block_hash(self) -> String {
        let block_string = format!(
            "{}{}{}{:?}{}",
            self.index, self.timestamp, self.merkle_root, self.transactions, self.previous_hash
        );
        let mut hasher = Sha256::new();
        hasher.update(&block_string);
        let result = hasher.finalize();
        result.iter().map(|byte| format!("{:02x}", byte)).collect()
    }

    fn calculate_merkle_root(&self) -> String {
        let mut transactions_hashes: Vec<String> = self
            .transactions
            .iter()
            .map(|transaction| transaction.calculate_hash())
            .collect();
        while transactions_hashes.len() > 1 {
            transactions_hashes = transactions_hashes
                .chunks_exact(2)
                .map(|chunk| {
                    let mut hasher = Sha256::new();
                    let concat = chunk.concat();
                    hasher.update(&concat);
                    let result = hasher.finalize();
                    result.iter().map(|byte| format!("{:02x}", byte)).collect()
                })
                .collect();
        }
        transactions_hashes.pop().unwrap_or_default()
    }
}

impl Default for DigiBlock {
    fn default() -> Self {
        Self {
            index: Default::default(),
            timestamp: Default::default(),
            merkle_root: Default::default(),
            transactions: Default::default(),
            previous_hash: Default::default(),
            hash: Default::default(),
            sign: Default::default(),
            proposed_by: Default::default(),
            proposals: Default::default(),
        }
    }
}

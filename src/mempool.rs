use crate::block::DigiBlock;
use crate::digichain::DigiChain;
use crate::proposal::{CrossChainWithdrawMsg, Proposal, ProposalType};
use crate::transaction::Transaction;
use crate::types::Address;
use cosmwasm_std::Uint128;
use ethers::types::Signature;
use rand::Rng;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::error::Error as StdError;
use std::sync::{Arc, RwLock};

#[derive(Clone, Debug)]
pub struct Mempool {
    pub proposals: Arc<RwLock<HashMap<String, Vec<Proposal>>>>,
    pub attested_idx: Arc<RwLock<HashMap<String, usize>>>,

    pub transactions: Vec<Transaction>,
    pub crosschain_request: HashMap<Address, Vec<CrossChainWithdrawMsg>>, // validator -> array of vector to process
}

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

impl Mempool {
    pub fn new() -> Mempool {
        Mempool {
            crosschain_request: Default::default(),
            attested_idx: Arc::new(RwLock::new(Default::default())),
            proposals: Arc::new(RwLock::new(Default::default())),
            transactions: vec![],
        }
    }
    pub fn get_mempool(&self) -> Mempool {
        self.clone()
    }

    pub fn add_crosschain_request(
        &mut self,
        validator: Address,
        withdraw_request: &CrossChainWithdrawMsg,
    ) {
        let ls = self.crosschain_request.get(&validator);
        if ls.is_none() {
            self.crosschain_request
                .insert(validator, vec![withdraw_request.clone()]);
        } else {
            let mut res = ls.unwrap().clone();
            res.push(withdraw_request.clone());
            self.crosschain_request.insert(validator, res);
        }
        println!(
            "CrossChain Withdraw Request Added ||  Src Nonce: {:?}",
            withdraw_request.src_nonce
        );
    }

    pub fn get_crosschain_request_to_execute(
        &self,
        validator: Address,
    ) -> Vec<CrossChainWithdrawMsg> {
        let ls = self.crosschain_request.get(&validator);
        if ls.is_none() {
            return vec![];
        }
        ls.unwrap().clone()
    }

    pub fn get_proposals(&self) -> Vec<Proposal> {
        let mut proposals = Vec::new();
        let pbinding = self.proposals.read().unwrap();
        for kv in pbinding.clone().into_iter() {
            for p in kv.1 {
                proposals.push(p);
            }
        }
        proposals
    }

    pub fn add_transaction(&mut self, tx: &Transaction) {
        // TODO: verify is proposal valid, signature is valid or not
        let tx_hash = tx.hash.clone();
        self.transactions.push(tx.clone());
        println!("Transaction Added ||  Hash: {:?}", tx_hash);
    }

    // broadcast to other validators after signing
    pub fn add_proposal(
        &mut self,
        proposal_type: ProposalType,
        mut proposal: Proposal,
    ) -> Result<bool, Box<dyn StdError>> {
        let mut proposals = self.proposals.write().unwrap();
        let mut proposals_arr: Vec<Proposal> = Vec::new();
        let res = proposals.get(&proposal_type.to_string());
        if res.is_some() {
            proposals_arr = res.unwrap().clone();
        }
        proposal.hash = proposal.calculate_hash();
        //TODO: sign proposal before pushing
        //TODO: check if same data present in arr or not, if it is already there then ignore it
        proposals_arr.push(proposal.clone());
        proposals.insert(proposal_type.to_string(), proposals_arr.clone());
        println!("Proposal Added ||  Hash: {:?}", proposal.hash);
        Ok(true)
    }

    pub fn select_txs_randomly(&mut self, digichain: DigiChain) -> Vec<Transaction> {
        if self.transactions.len() == 0usize {
            return vec![];
        }
        let mut rng = rand::thread_rng();
        let mut indices: Vec<usize> = (0..self.transactions.len()).collect();
        // indices.shuffle(&mut rng);
        let mut subset_length = rng.gen_range(1..=20);
        if subset_length > self.transactions.len() {
            subset_length = self.transactions.len();
        }

        let selected_transactions: Vec<Transaction> = indices
            .into_iter()
            .take(subset_length)
            .filter_map(|index| {
                let transaction: Transaction = self.transactions[index].clone();
                if transaction.is_valid(digichain.clone()) {
                    Some(transaction)
                } else {
                    // If not valid, remove from self.transactions
                    // and return None (not included in the result)
                    self.transactions.remove(index);
                    None
                }
            })
            .collect();
        selected_transactions
    }

    pub fn select_proposals_randomly(&mut self, digichain: DigiChain) -> Vec<Proposal> {
        let proposals = self.get_proposals();
        if proposals.len() == 0usize {
            return vec![];
        }
        let mut rng = rand::thread_rng();
        let mut indices: Vec<usize> = (0..proposals.len()).collect();
        // indices.shuffle(&mut rng);

        let mut subset_length = rng.gen_range(1..=20);
        if subset_length > proposals.len() {
            subset_length = proposals.len();
        }
        // remove included proposol
        let mut delete_mp: HashMap<String, Vec<String>> = HashMap::new();
        let selected_proposals: Vec<Proposal> = indices
            .into_iter()
            .take(subset_length)
            .filter_map(|index| {
                let proposal: Proposal = proposals[index].clone();
                if proposal.is_valid(digichain.clone()) {
                    Some(proposal)
                } else {
                    // If not valid, remove from self.proposals
                    // and return None (not included in the result)
                    let mut uarr: Vec<String> = Vec::new();
                    let res = delete_mp.get(&proposal.proposal_type.to_string());
                    if res.is_some() {
                        uarr = res.unwrap().clone();
                    }
                    uarr.push(proposal.clone().hash);
                    delete_mp.insert(proposal.proposal_type.to_string(), uarr.clone());
                    None
                }
            })
            .collect();

        self.drop_proposals(delete_mp);
        selected_proposals
    }

    pub fn select_txs_and_proposals_randomly(
        &mut self,
        digichain: DigiChain,
    ) -> (Vec<Transaction>, Vec<Proposal>) {
        (
            self.select_txs_randomly(digichain.clone()),
            self.select_proposals_randomly(digichain),
        )
    }

    pub fn drop_tx_and_proposals(&mut self, block: DigiBlock) {
        // remove included tx
        let included_tx_hashes: Vec<String> = block
            .clone()
            .get_transactions()
            .iter()
            .cloned()
            .map(|tx| tx.hash)
            .collect();
        // Retain only transactions that are not included in the block
        self.transactions
            .retain(|tx| !included_tx_hashes.contains(&&tx.hash));

        // remove included proposol
        let mut delete_mp: HashMap<String, Vec<String>> = HashMap::new();
        let _: Vec<bool> = block
            .get_proposals()
            .iter()
            .cloned()
            .map(|proposal: Proposal| {
                let mut uarr: Vec<String> = Vec::new();
                let res = delete_mp.get(&proposal.proposal_type.to_string());
                if res.is_some() {
                    uarr = res.unwrap().clone();
                }
                // proposal.hash
                uarr.push(proposal.hash);
                delete_mp.insert(proposal.proposal_type.to_string(), uarr.clone());
                true
            })
            .collect();
        self.drop_proposals(delete_mp);
        // Retain only transactions that are not included in the block
    }

    pub fn drop_proposals(&mut self, delete_mp: HashMap<String, Vec<String>>) {
        let mut proposals = self.proposals.write().unwrap();
        for kv in delete_mp.clone().into_iter() {
            let tproposals = proposals.get_mut(&kv.0).unwrap();
            tproposals.retain(|p| {
                if kv.1.contains(&&p.hash) {
                    return false;
                }
                true
            })
        }
    }
}

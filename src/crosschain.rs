use crate::{
    types::{Address, ContractConfig},
    utils::address_to_str,
};
use cosmwasm_std::Uint128;
use serde::{Deserialize, Serialize};
use std::error::Error as StdError;
use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub enum CrossChainExecutionReply {
    Error(String),
    Result(CrossChainExecutionResult),
    None,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct CrossChainExecutionResult {
    pub src_tx_hash: String,
    pub dst_tx_hash: Option<String>,
    pub ack_tx_hash: Option<String>, // exist in case of failure during minting funds here
                                     //TODO: result or status of cross chain request can be added
}

#[derive(Clone, Debug)]
pub struct CrossChain {
    pub self_chain_id: String,
    pub self_nonce: Uint128,
    pub tmp_idx_mp: Arc<RwLock<HashMap<(String, String), usize>>>, // string,nonce: idx proposal, as of now chain,nonce, should be mapped hash value of data -> idx
    pub contract_configs: Arc<RwLock<HashMap<String, ContractConfig>>>, // vector of chain_ids supported here
    pub requests: Arc<RwLock<HashMap<(String, String), CrossChainExecutionResult>>>, // (src_chain_id,nonce) -> CrossChainExecutionResult

    pub broadcasted: Arc<
        RwLock<
            HashMap<(String, String), HashMap<Address, bool>>, // (src_chain_id,nonce) => Validator -> true
        >,
    >,
}

impl CrossChain {
    pub fn new(
        self_chain_id: String,
        tmp_idx_mp: HashMap<(String, String), usize>, // chain_id,nonce: idx crosschain request
    ) -> CrossChain {
        CrossChain {
            self_nonce: Uint128::from(0u128),
            self_chain_id,
            tmp_idx_mp: Arc::new(RwLock::new(tmp_idx_mp)),
            contract_configs: Arc::new(RwLock::new(HashMap::new())),
            requests: Arc::new(RwLock::new(HashMap::new())),
            broadcasted: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub fn broadcasted(
        &mut self,
        validator: Address,
        src_chain_id: String,
        src_nonce: Uint128,
    ) -> Result<bool, Box<dyn StdError>> {
        let mut binding = self.broadcasted.write().unwrap();
        let res = binding.get_mut(&(src_chain_id.clone(), src_nonce.to_string()));
        let vhmp;
        if res.is_none() {
            binding.insert(
                (src_chain_id.clone(), src_nonce.to_string()),
                HashMap::new(),
            );
            vhmp = binding
                .get_mut(&(src_chain_id, src_nonce.to_string()))
                .unwrap();
        } else {
            vhmp = res.unwrap();
        }
        let res = vhmp.get(&validator);
        if res.is_some() {
            if res.unwrap().clone() == true {
                return Err(format!("Already Broadcasted").into());
            }
        }
        vhmp.insert(validator, true);
        Ok(true)
    }

    pub fn is_broadcasted(
        &self,
        validator: Address,
        src_chain_id: String,
        src_nonce: Uint128,
    ) -> bool {
        let binding = self.broadcasted.read().unwrap();
        let res = binding.get(&(src_chain_id, src_nonce.to_string()));
        if res.is_none() {
            return false;
        }
        let res = res.unwrap();
        let res = res.get(&validator);
        if res.is_none() {
            return false;
        }
        return res.unwrap().clone() == true;
    }

    pub fn get_request(
        &self,
        src_chain_id: String,
        src_nonce: Uint128,
    ) -> Result<CrossChainExecutionResult, Box<dyn StdError>> {
        let binding = self.requests.write().unwrap();
        let res = binding.get(&(src_chain_id, src_nonce.to_string()));
        if res.is_none() {
            return Err(format!("request not found").into());
        }
        Ok(res.unwrap().clone())
    }

    pub fn add_request(
        &mut self,
        src_chain_id: String,
        src_nonce: Uint128,
        cce_result: CrossChainExecutionResult,
    ) -> bool {
        let mut binding = self.requests.write().unwrap();
        binding.insert((src_chain_id, src_nonce.to_string()), cce_result);
        true
    }

    pub fn update_contract_config(
        &mut self,
        src_chain_id: String,
        last_processed_nonce: Uint128,
        last_processed_block: u64,
    ) -> Result<ContractConfig, Box<dyn StdError>> {
        let mut binding = self.contract_configs.write().unwrap();
        let res = binding.get(&src_chain_id);
        if res.is_none() {
            return Err("contract config not found".into());
        }
        let mut res = res.unwrap().clone();
        res.last_proccessed_nonce = last_processed_nonce;
        res.last_processed_block = last_processed_block;
        binding.insert(src_chain_id, res.clone());
        Ok(res.clone())
    }

    pub fn get_ccr_idx(
        &self,
        chain_id: String,
        nonce: Uint128,
    ) -> Result<usize, Box<dyn StdError>> {
        let binding = self.tmp_idx_mp.read().unwrap();
        let res = binding.get(&(chain_id, nonce.to_string()));
        if res.is_none() {
            return Err("cc_request not exist, so idx".into());
        }
        Ok(res.unwrap().clone())
    }

    pub fn increase_nonce(&mut self) -> Uint128 {
        self.self_nonce = self.self_nonce + Uint128::from(1u128);
        self.self_nonce
    }

    pub fn is_contract_registered(&self, chain_id: String, contract: Address) -> bool {
        let binding = self.contract_configs.read().unwrap();
        if !binding.contains_key(&chain_id) {
            return false;
        }
        if binding.get(&chain_id).unwrap().contract_address != address_to_str(contract) {
            return false;
        }
        true
    }

    pub fn get_contracts_config(
        &self,
        mut chain_ids: Vec<String>,
    ) -> HashMap<String, ContractConfig> {
        let mut configs = HashMap::new();
        if chain_ids.len() == 0usize {
            chain_ids = self
                .contract_configs
                .read()
                .unwrap()
                .keys()
                .into_iter()
                .map(|chain_id| chain_id.clone())
                .collect::<Vec<String>>();
        }
        let binding = self.contract_configs.read().unwrap();
        let _ = chain_ids
            .into_iter()
            .map(|chain_id| {
                let res = binding.get(&chain_id);
                if res.is_some() {
                    configs.insert(chain_id, res.unwrap().clone());
                    return true;
                }
                return false;
            })
            .collect::<Vec<bool>>();
        configs
    }

    pub fn add_contract_config(
        &mut self,
        chain_id: String,
        contract_address: String,
        start_block: u64,
        chain_type: u8,
    ) -> Result<bool, Box<dyn StdError>> {
        let config = ContractConfig {
            contract_address: contract_address.to_lowercase(),
            start_block,
            last_proccessed_nonce: Uint128::from(0u128),
            last_processed_block: start_block,
            chain_type,
        };
        let mut binding = self.contract_configs.write().unwrap();
        if binding.contains_key(&chain_id) {
            return Err(format!("contract config already present").into());
        }
        binding.insert(chain_id, config);
        return Ok(true);
    }
}

impl Default for CrossChain {
    fn default() -> Self {
        Self {
            self_nonce: Uint128::from(0u128),
            self_chain_id: Default::default(),
            tmp_idx_mp: Default::default(),
            contract_configs: Default::default(),
            requests: Default::default(),
            broadcasted: Default::default(),
        }
    }
}

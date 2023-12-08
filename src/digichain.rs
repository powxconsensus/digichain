use crate::{
    acccount::Account,
    block::DigiBlock,
    crosschain::CrossChain,
    json_rpc::JsonRpc,
    mempool::Mempool,
    proposal::{CrossChainWithdrawMsg, ExtraData, Proposal, ProposalType},
    token::DigiToken,
    transaction::{Transaction, TxType},
    types::{Address, HexString, TokenId, TxExecutionResult},
    utils::encode_crosschain_request_type_data,
    validators::Validator,
};
use actix_web::web;
use cosmwasm_std::Uint128;
use ethers::types::{Signature, U256 as EthU256};
use jsonrpc_http_server::jsonrpc_core::{Params, Value};
use rand::Rng;
use router_wasm_bindings::ethabi::{
    decode, encode, ethereum_types::U256, Address as EthRouterAddress, ParamType, Token,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    error::Error,
    fmt::format,
    str::FromStr,
    sync::{Arc, Mutex, RwLock},
    time::SystemTime,
};

#[derive(Clone, Debug)]
pub struct DigiChain {
    pub validators: Arc<RwLock<Vec<Validator>>>,
    pub chain_id_to_token_mp: Arc<RwLock<HashMap<(String, String), TokenId>>>, // chain_id, contract_dddress : token_address on chain with chainid chain_id
    pub chain_id: String,
    pub mempool: Arc<RwLock<Mempool>>,
    pub json_rpc: Arc<RwLock<JsonRpc>>,
    pub validator: Arc<RwLock<Validator>>,
    pub blocks: Arc<RwLock<Vec<Arc<RwLock<DigiBlock>>>>>,
    // pub token_list: Arc<RwLock<HashMap<String, Arc<RwLock<DigiToken>>>>>, // chain_id : contract address : token_address on chain with chainid chain_id
    pub token_list: Arc<RwLock<HashMap<String, DigiToken>>>, // chain_id : contract address : token_address on chain with chainid chain_id
    pub accounts: Arc<RwLock<HashMap<Address, Arc<RwLock<Account>>>>>, // address -> account
    pub crosschain: Arc<RwLock<CrossChain>>,

    pub index_transactions: Arc<RwLock<HashMap<String, usize>>>, // tx hash -> block number
    pub index_proposals: Arc<RwLock<HashMap<String, usize>>>,    // tx hash -> block number
    pub pause: Arc<RwLock<bool>>,
}

impl Default for DigiChain {
    fn default() -> DigiChain {
        return DigiChain {
            pause: Arc::new(RwLock::new(false)),
            index_transactions: Arc::new(RwLock::new(HashMap::new())),
            index_proposals: Arc::new(RwLock::new(HashMap::new())),
            validators: Arc::new(RwLock::new(Default::default())),
            chain_id_to_token_mp: Arc::new(RwLock::new(HashMap::new())),
            chain_id: String::default(),
            mempool: Arc::new(RwLock::new(Mempool::new())),
            validator: Arc::new(RwLock::new(Validator::default())),
            json_rpc: Arc::new(RwLock::new(JsonRpc::new())),
            blocks: Arc::new(RwLock::new(vec![])),
            token_list: Arc::new(RwLock::new(HashMap::new())),
            accounts: Arc::new(RwLock::new(HashMap::new())),
            crosschain: Arc::new(RwLock::new(Default::default())),
        };
    }
}

impl DigiChain {
    ///////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
    /////////////////////////////////////////////////Digi Chain////////////////////////////////////////////////////////////////////////////
    ///////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
    pub async fn add_block(&mut self, block: DigiBlock) {
        // if cid_result.is_err() {
        //     return Err(w3s::helper::Error::UploadError("s"));
        // }
        //store it in

        //TODO: check for consensus
        self.blocks
            .write()
            .unwrap()
            .push(Arc::new(RwLock::new(block)));
        // Ok(vec![])
    }

    pub fn get_block(&self, block_number: u64) -> Result<DigiBlock, Box<dyn Error>> {
        let binding = self.blocks.read().unwrap();
        let res = binding.get(block_number as usize);
        if res.is_none() {
            return Err("block not found".into());
        }
        let res = res.unwrap().read().unwrap();
        Ok(res.clone())
    }

    pub fn get_block_number(&self) -> u64 {
        return self.blocks.read().unwrap().len() as u64;
    }
    pub fn get_no_of_tokens(&self) -> u64 {
        return self.token_list.read().unwrap().len() as u64;
    }

    pub(crate) fn get_random_validator(&self) -> Address {
        let binding = self.validators.read().unwrap();
        let mut rng = rand::thread_rng();
        let index = rng.gen_range(0..binding.len());
        return binding.get(index).unwrap().acccount.address;
    }

    // it adds validator data into packet
    pub(crate) fn get_cmp_ccr_data(
        &self,
        crosschain: &mut std::sync::RwLockWriteGuard<'_, CrossChain>,
        request_type: u8,
        src_chain_id: String,
        dst_chain_id: String,
        mut src_nonce: Uint128,
        mut dst_nonce: Uint128,
    ) -> HexString {
        if dst_chain_id != self.chain_id {
            // it means request coming from other chain
            src_nonce = crosschain.increase_nonce(); // increase nonce and return latest nonce
        }
        let binding = self.validators.read().unwrap();
        let mut rng = rand::thread_rng();
        let index = rng.gen_range(0..binding.len());
        let validator = binding.get(index).unwrap().acccount.address;
        // convert to token to pack
        return encode_crosschain_request_type_data(
            request_type,
            src_chain_id,
            src_nonce,
            dst_chain_id,
            dst_nonce,
            validator,
        );
    }

    pub(crate) fn add_validator(&mut self, validator: Validator) -> bool {
        let mut binding = self.validators.write().unwrap();
        binding.push(validator);
        true
    }
    pub(crate) fn get_token(&self, from: usize, to: usize) -> Vec<DigiToken> {
        let token_list = self.token_list.read().unwrap();
        let token_ids = token_list.keys();
        let token_ids: Vec<String> = token_ids.map(|id| id.clone()).collect();
        if token_ids.len() == 0 {
            return vec![];
        }
        token_ids[from..to]
            .into_iter()
            .map(|id| token_list.get(id).unwrap().clone())
            .collect::<Vec<DigiToken>>()
    }

    pub(crate) fn get_previous_hash(&self) -> String {
        return self.get_block(self.get_block_number() - 1).unwrap().hash;
    }
    pub(crate) fn get_chain(&self, start_block: usize, end_block: usize) -> Vec<DigiBlock> {
        return self.blocks.read().unwrap()[start_block..end_block]
            .into_iter()
            .map(|ablock| ablock.read().unwrap().clone())
            .collect();
    }
    pub(crate) fn get_chain_id(&self) -> String {
        return self.chain_id.clone();
    }
    ///////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
    /////////////////////////////////////////////////Digi Validators///////////////////////////////////////////////////////////////////////
    ///////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
    pub(crate) fn execute_proposals(
        &mut self,
        block_number: u64,
        timestamp: u64,
        proposals: Vec<Proposal>,
    ) -> Vec<Proposal> {
        let mut executed_proposals: Vec<Proposal> = Vec::new();
        for mut proposal in proposals {
            let original_chain_state = self.clone();
            // proposal
            let res = proposal.execute(block_number, timestamp, self);
            if res.is_err() {
                // revert chain state
                *self = original_chain_state;
                proposal.result = TxExecutionResult::Error(format!("{:?}", res.err()));
            } else {
                proposal.result = TxExecutionResult::Result(format!("{:?}", res.unwrap()));
            }
            proposal.timestamp = timestamp;
            proposal.block_number = block_number;

            //increase proposal nonce of from account
            let mut binding = self.accounts.write().unwrap();
            let mut account = binding.get(&proposal.proposed_by);
            if account.is_none() {
                binding.insert(
                    proposal.proposed_by,
                    Arc::new(RwLock::new(Account::new(proposal.proposed_by))),
                );
                account = binding.get(&proposal.proposed_by);
            }
            let account = account.unwrap();
            account.write().unwrap().increase_proposal_nonce();

            // index it
            self.index_proposals
                .write()
                .unwrap()
                .insert(proposal.hash.clone(), proposal.block_number as usize);
            executed_proposals.push(proposal);
        }
        executed_proposals
    }

    pub(crate) fn execute_txs(
        &mut self,
        block_number: u64,
        timestamp: u64,
        txs: Vec<Transaction>,
    ) -> Vec<Transaction> {
        let mut executed_txs: Vec<Transaction> = Vec::new();
        for mut tx in txs {
            let original_chain_state = self.clone();
            // proposal
            let res = tx.execute(block_number, timestamp, self);
            if res.is_err() {
                // revert chain state
                *self = original_chain_state;
                tx.result = TxExecutionResult::Error(format!("{:?}", res.err()));
            } else {
                tx.result = TxExecutionResult::Result(format!("{:?}", res.unwrap()));
            }
            tx.timestamp = timestamp;
            tx.block_number = block_number;

            //increase  nonce of from account
            let mut binding = self.accounts.write().unwrap();
            let mut account = binding.get(&tx.from);
            if account.is_none() {
                binding.insert(tx.from, Arc::new(RwLock::new(Account::new(tx.from))));
                account = binding.get(&tx.from);
            }
            let account = account.unwrap();
            account.write().unwrap().increase_tx_nonce();

            // index it
            self.index_transactions
                .write()
                .unwrap()
                .insert(tx.hash.clone(), tx.block_number as usize);

            account
                .write()
                .unwrap()
                .add_transaction_hash(tx.hash.clone());
            executed_txs.push(tx);
        }
        executed_txs
    }

    pub async fn add_blocks(&mut self) {
        loop {
            if self.pause.read().unwrap().clone() {
                tokio::time::sleep(std::time::Duration::from_secs(3)).await;
                continue;
            }
            self.attest_proposals().await;
            let timestamp = SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_secs();
            let block_number = self.get_block_number();
            let mut txs: Vec<Transaction>;
            let mut proposals: Vec<Proposal>;
            {
                let mut mempool = self.mempool.write().unwrap();
                // txs = mempool.select_txs_randomly(self.clone());
                (txs, proposals) = mempool.select_txs_and_proposals_randomly(self.clone());
                // fetch proposals which have been validated
            }
            // execute all tx and proposals
            // TODO: tx will be executed in any other, but it should executed in increasing order of nonce of from account
            //TODO: update block_number of txs and proposal with block_number
            txs = self.execute_txs(block_number, timestamp, txs);
            proposals = self.execute_proposals(block_number, timestamp, proposals);
            let block = DigiBlock::create_block(
                self.validator.read().unwrap().clone(),
                timestamp,
                block_number,
                self.get_previous_hash(),
                txs,
                proposals, //TODO: implement execution of proposols
            );
            self.add_block(block.clone()).await;
            self.mempool.write().unwrap().drop_tx_and_proposals(block);
            // block after every 3sec
            tokio::time::sleep(std::time::Duration::from_secs(3)).await;
        }
    }

    pub(crate) async fn attest_proposals(&mut self) {
        let mempool = self.mempool.write().unwrap();
        let mut proposals_binding = mempool.proposals.write().unwrap();
        let validator = self.validator.read().unwrap();
        for mut kv in proposals_binding.clone().into_iter() {
            let pidx_mp_binding = mempool.attested_idx.write().unwrap();
            let res = pidx_mp_binding.get(&kv.0.clone());
            let mut idx = 0usize;
            if res.is_some() {
                idx = res.unwrap().clone();
            }
            while idx < kv.1.len() {
                let res = kv.1.get_mut(idx);
                let proposal = res.unwrap();
                if !proposal.is_signed(validator.acccount.address) {
                    //TODO: sign tx
                    println!("Signed Proposal || TxHash: {}", proposal.hash);
                    proposal.validtors_signature.push((
                        validator.acccount.address,
                        Signature {
                            r: EthU256::default(),
                            s: EthU256::default(),
                            v: 123u64,
                        },
                    ));

                    let mut variant = proposal.proposal_type.to_string();
                    if let Some(s) = variant.find('(') {
                        variant = variant[..s].to_string();
                    }
                    // if crosschain request to other chain
                    if variant == "CrossChainRequest".to_string() {
                        let extra_data_res: Result<
                            CrossChainWithdrawMsg,
                            Box<dyn std::error::Error>,
                        > = match &proposal.extra_data {
                            Some(ExtraData::WithdrawData(data)) => Ok(CrossChainWithdrawMsg {
                                dst_chain_id: data.dst_chain_id.clone(),
                                src_chain_id: data.src_chain_id.clone(),
                                src_nonce: data.src_nonce,
                                payload: data.payload.clone(),
                                sigs: data.sigs.clone(),
                            }),
                            Some(_) => Err("unknown extra data".into()),
                            None => Err("missing extra data".into()),
                        };
                        if extra_data_res.is_err() {
                            continue;
                        }
                        let mut withdraw_data = extra_data_res.unwrap();
                        withdraw_data.sigs.push(Signature {
                            r: EthU256::default(),
                            s: EthU256::default(),
                            v: 123u64,
                        });
                        proposal.extra_data = Some(ExtraData::WithdrawData(withdraw_data));
                    }
                }
                idx = idx + 1usize;
            }
            proposals_binding.insert(kv.0, kv.1);
        }
    }

    ///////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
    /////////////////////////////////////////////////Digi Token////////////////////////////////////////////////////////////////////////////
    ///////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
    pub(crate) fn get_all_tokens(self) -> HashMap<String, DigiToken> {
        return self.token_list.read().unwrap().clone();
    }

    pub(crate) fn get_token_by_id(&self, id: String) -> Result<DigiToken, Box<dyn Error>> {
        let binding = self.token_list.read().unwrap();
        let res = binding.get(&id);
        if res.is_none() {
            return Err("token not found with given chain_id".into());
        }
        return Ok(res.unwrap().clone());
    }

    ///////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
    /////////////////////////////////////////////////Digi Token////////////////////////////////////////////////////////////////////////////
    ///////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
    pub fn get_account(&self, address: Address) -> Result<Account, Box<dyn std::error::Error>> {
        let binding = self.accounts.read().unwrap();
        let res = binding.get(&address);
        if res.is_none() {
            return Err(format!("Account Not Found!!").into());
        }
        return Ok(res.unwrap().read().unwrap().clone());
    }
}

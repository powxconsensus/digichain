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

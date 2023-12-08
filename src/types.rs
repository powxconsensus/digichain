use std::collections::HashMap;

use crate::{
    proposal::{ProposalType, RawProposal},
    transaction::{RawTransaction, Transaction},
};
use cosmwasm_std::Uint128;
use ethers::{types::Signature, utils::hex::FromHexError};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub type Address = ethereum_types::Address;

pub type TokenId = String;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]

pub struct GetBlockParams {
    pub block_number: u64,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]

pub struct TxResponse {
    pub tx_hash: u64,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct GetChainParams {
    pub start_block: u64,
    pub end_block: u64,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct BroadcastTransactionParams {
    pub transaction: RawTransaction,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct BroadcastProposalParams {
    pub proposal: RawProposal,
}

// #[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
// pub struct TransactionParams {
//     pub hash: String,
//     pub created_at: u64,
//     pub nonce: Uint128,
//     pub from: Address,
//     pub to: Address,
//     pub tokens: Vec<String>,
//     pub data: Vec<Vec<u8>>,
//     pub amount: Uint128,
//     pub slippage: f32,
//     pub refund_token: String,
//     pub signature: Signature,
//     pub chain_id: String,
// }

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct GetAccountParams {
    pub address: Address,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub enum TxExecutionResult {
    Result(String),
    Error(String),
    None,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct KYCParams {
    pub upi_id: String,
    pub name: String,
    pub address: String,
    pub aadhar_no: String,
    pub mobile: String,
    pub country: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct TokenAcceptsParams {
    pub tokens: Vec<String>,
    pub amounts: Vec<Uint128>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct HexString(String);

impl HexString {
    pub fn new(hex_string: String) -> Self {
        let mut hex_str = hex_string;
        if hex_str.starts_with("0x") {
            hex_str = String::from(&hex_str[2..]);
        }
        HexString(hex_str)
    }

    pub fn to_vec(&self) -> Result<Vec<u8>, FromHexError> {
        let mut hex_str = self.0.clone();
        if hex_str.starts_with("0x") {
            hex_str = String::from(&hex_str[2..]);
        }
        hex::decode(&hex_str)
    }

    pub fn from_vec(data: Vec<u8>) -> HexString {
        HexString(hex::encode(data))
    }

    pub fn from_str(fstr: &str) -> HexString {
        HexString(fstr.to_string())
    }
}

impl Default for HexString {
    fn default() -> Self {
        HexString(Default::default())
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct GetTokenParams {
    pub token_id: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct GetTokenByChain {
    pub chain_id: String,
    pub token_address: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct GetTokensParams {
    pub from: Option<u64>,
    pub to: Option<u64>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct AddTokenParams {
    pub name: String,
    pub symbol: String,
    pub decimal: u8,
    pub price: Uint128,                               // dollar value * 10^9
    pub chain_token_mapping: HashMap<String, String>, // chain_id : token_address on chain with chainid chain_id
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct AddContractConfigParams {
    pub contract_address: String,
    pub start_block: u64,
    pub chain_id: String,
    pub chain_type: u8,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct UpdateTokensPriceParams {
    pub tokens: Vec<TokenId>,
    pub prices: Vec<Uint128>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct ContractConfig {
    pub contract_address: String,
    pub start_block: u64,
    pub last_processed_block: u64,
    pub last_proccessed_nonce: Uint128,
    pub chain_type: u8,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct GetConfigParams {
    pub chain_ids: Vec<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct CrossChainRequestParams {
    pub src_tx_hash: String,
    pub src_chain_id: String,
    pub dest_chain_id: String,
    pub src_contract: String,
    pub msg: HexString,
    pub src_nonce: Uint128,
    pub depositor: Address,
    pub src_block_number: u64,
    pub created_at: u64,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct GetCrossChainRequestsParams {
    pub from: Option<u64>,
    pub to: Option<u64>,
    pub src_chain_id: Option<String>,
    pub src_nonce: Option<Uint128>,
    pub dest_chain_id: Option<String>,
}

////////////////////////////////////////////////////////////

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct TxTransferParams {
    pub to: Address,          //
    pub tokens: Vec<TokenId>, //
    pub data: Vec<HexString>, //
    pub amount: Uint128,
    pub slippage: Uint128,
    pub refund_token: TokenId, //
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct TxCrossChainTransferParams {
    pub recipient: Address,   // recipient
    pub tokens: Vec<TokenId>, //
    pub data: Vec<HexString>, //
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct GetProposalsParams {
    pub from: Option<u64>,
    pub to: Option<u64>,
    pub hash: Option<String>,
    pub proposal_type: Option<ProposalType>,
    pub proposed_by: Option<Address>,
    pub block_number: Option<u64>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct TxCrossChainRequestParams {
    pub src_chain_id: String,
    pub dst_chain_id: String, // dst_chain_id
    pub src_contract: Address,
    pub recipient: Address,    // recipient
    pub depositor: Address,    // depositor
    pub tokens: Vec<Address>,  //
    pub amounts: Vec<Uint128>, //
    pub src_nonce: Uint128,
    pub src_block_number: u64,
    pub src_tx_hash: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct GetBalanceOf {
    pub token_id: String,
    pub address: Address,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct IsBroadcastedParams {
    pub validator: Address,
    pub src_chain_id: String,
    pub src_nonce: Uint128,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct GetBalances {
    pub tokens: Vec<String>,
    pub addresses: Vec<Address>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct GetTransactionParams {
    pub tx_hash: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct GetTransactionsParams {
    pub address: Option<Address>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct GetCrossChainRequestReadyToExecute {
    pub validator: Address,
    pub from: Option<u64>,
    pub to: Option<u64>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct AirDropParams {
    pub address: Address,
    pub token: String,
    pub amount: Uint128,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct PauseAndUnPauseParams {
    pub pause: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct GetOptimalPath {
    pub tokens: Vec<String>,
    pub amounts: Vec<Uint128>,
    pub amount: Uint128, // dollar multiplied by 10^9
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct CrossChainRequestTypeData {
    pub request_type: u8,
    pub src_chain_id: String,
    pub src_nonce: Uint128,
    pub dst_chain_id: String,
    pub dst_nonce: Uint128,
    pub validator: Address,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct TxCrossChainReplyParams {
    pub src_chain_id: String,
    pub dst_chain_id: String, // dst_chain_id
    pub src_contract: Address,
    pub recipient: Address,    // recipient
    pub depositor: Address,    // depositor
    pub tokens: Vec<Address>,  //
    pub amounts: Vec<Uint128>, //
    pub src_nonce: Uint128,
    pub dst_nonce: Uint128,
    pub dst_block_number: u64,
    pub dst_tx_hash: String,
}

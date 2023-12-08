use crate::{
    constants::UnLockedWithdrawRequest,
    crosschain,
    digichain::DigiChain,
    proposal::{CrossChainWithdrawMsg, ExtraData, Proposal, ProposalType},
    token::{decode_crosschain_tx_data, DigiToken},
    types::{Address, HexString, TokenAcceptsParams, TxExecutionResult},
    utils::{
        decode_crosschain_request_type_data, get_crosschain_transfer_payload_params,
        get_crosschain_withdraw_payload, get_transfer_payload_params, is_within_slippage,
    },
};
use cosmwasm_std::Uint128;
use ethers::types::Signature;
use router_wasm_bindings::ethabi::{
    decode, encode, Address as EthRouterAddress, Error as EthError, ParamType, Token,
};
use serde::{Deserialize, Deserializer, Serialize};
use std::error::Error as StdError;

#[derive(Serialize, Clone, Debug, PartialEq)]
pub enum TxType {
    Transfer,
    CrosschainTransfer(String), // dst chain [created from this to other chain] and recipient on that chain
    CrossChainRequest(HexString), // src_chain,src_nonce, dst_chainId, validator assigned to execute this
    UserKYC,
    None,
    AddContractConfig,
    AddToken,
    UpdateTokenAccepts,
    UpdateTokensPrice,
}

impl TxType {
    pub fn to_string(&self) -> String {
        format!("{:?}", self)
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct RawTransaction {
    pub hash: String,
    pub created_at: u64,
    pub nonce: Uint128, // sender account nonce
    pub from: Address,
    pub tx_type: TxType,
    pub signature: Signature,
    pub chain_id: String,
    pub data: HexString,
}

impl RawTransaction {
    pub fn to_transaction(self) -> Transaction {
        Transaction {
            chain_id: self.chain_id,
            nonce: self.nonce,
            hash: self.hash,
            created_at: self.created_at,
            timestamp: 0,
            from: self.from,
            data: self.data,
            signature: self.signature,
            block_number: 0,
            result: TxExecutionResult::None,
            tx_type: self.tx_type,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Transaction {
    pub hash: String,
    pub created_at: u64,
    pub chain_id: String,
    pub timestamp: u64, // mined at
    pub nonce: Uint128,
    pub from: Address,
    pub data: HexString,
    pub signature: Signature,
    pub block_number: u64,
    pub result: TxExecutionResult,
    pub tx_type: TxType,
}

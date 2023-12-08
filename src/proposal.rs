use crate::{
    acccount::Account,
    constants::{LockedFundRequest, UnLockedFailedRequest, UnLockedWithdrawRequest},
    crosschain::CrossChainExecutionResult,
    digichain::DigiChain,
    token::DigiToken,
    types::{
        AddContractConfigParams, AddTokenParams, Address, CrossChainRequestParams, HexString,
        KYCParams, TxCrossChainReplyParams, TxCrossChainRequestParams, TxExecutionResult,
        UpdateTokensPriceParams,
    },
    utils::{
        address_to_str, decode_crosschain_request_type_data, encode_crosschain_request_type_data,
    },
};
use cosmwasm_std::Uint128;

use ethers::{
    abi::Tokenizable,
    types::{Signature, U256},
    utils::hex::encode,
};
use router_wasm_bindings::ethabi::{
    decode, encode as rencode, Address as EthRouterAddress, ParamType, Token,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use sha2::digest::typenum::Len;
use std::{
    cmp::Ordering,
    collections::HashMap,
    error::Error as StdError,
    str::FromStr,
    string,
    sync::{Arc, RwLock},
    vec,
};

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum ProposalType {
    UserKYC,
    CrossChainRequest(HexString), // src_chain,src_nonce, dst_chainId, validator assigned to execute this
    UpdateToken,
    AddValidators,
    RemoveValidators,
    AddToken,
    AddChainToken,
    UpdateChainToken,
    AddContractConfig,
    UpdateTokensPrice,
    None,
}

impl ProposalType {
    pub fn to_string(&self) -> String {
        format!("{:?}", self)
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ProposalHashMake {
    pub chain_id: String,
    pub proposal_type: ProposalType,
    pub data: HexString,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct RawProposal {
    pub hash: String,
    pub chain_id: String,
    pub proposal_type: ProposalType,
    pub proposed_by: Address,
    pub proposed_at: u64,
    pub data: HexString,
    pub nonce: Uint128,
    pub signature: Signature,
    pub extra_data: Option<ExtraData>,
}

impl RawProposal {
    pub fn to_proposal(&self) -> Proposal {
        Proposal {
            hash: self.hash.clone(),
            chain_id: self.chain_id.clone(),
            proposal_type: self.proposal_type.clone(),
            proposed_by: self.proposed_by,
            proposed_at: self.proposed_at,
            data: self.data.clone(),
            nonce: self.nonce,
            block_number: Default::default(),
            signature: self.signature,
            validtors_signature: vec![(self.proposed_by, self.signature)],
            timestamp: Default::default(),
            result: TxExecutionResult::None,
            extra_data: self.extra_data.clone(),
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct CrossChainWithdrawMsg {
    pub dst_chain_id: String,
    pub src_chain_id: String,
    pub src_nonce: Uint128,
    pub payload: HexString,
    pub sigs: Vec<Signature>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum ExtraData {
    WithdrawData(CrossChainWithdrawMsg),
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Proposal {
    pub hash: String,
    pub chain_id: String,
    pub proposal_type: ProposalType,
    pub proposed_by: Address,
    pub proposed_at: u64,
    pub data: HexString,
    pub nonce: Uint128,
    pub block_number: u64,
    pub signature: Signature,
    pub validtors_signature: Vec<(Address, Signature)>,
    pub timestamp: u64, // mined at
    pub result: TxExecutionResult,
    pub extra_data: Option<ExtraData>,
}

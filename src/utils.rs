use crate::{
    block::DigiBlock,
    digichain::DigiChain,
    token::DigiToken,
    types::{
        CrossChainRequestTypeData, HexString, TokenId, TxCrossChainTransferParams,
        TxExecutionResult, TxTransferParams,
    },
    validators::Validator,
};
use cosmwasm_std::Uint128;
use ethers::signers::{LocalWallet, Wallet};
use ethers::types::Signature;
use ethers::{contract, core::types::transaction::eip712::Eip712};
use ethers::{prelude::*, utils::hash_message};
use router_wasm_bindings::ethabi::{
    decode, encode, ethereum_types::U256, Address as EthRouterAddress, ParamType, Token,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::convert::TryInto;
use std::error::Error as StdError;

pub fn is_within_slippage(
    user_paid_amount: Uint128,
    slippage: Uint128,              // it can be upto 4decimal
    resulted_tokens_price: Uint128, // is multiple of 10^9
) -> bool {
    let mut lower_limit = Uint128::from(0u128);
    if user_paid_amount > slippage {
        lower_limit = user_paid_amount - slippage;
    }
    let upper_limit = user_paid_amount + slippage;
    resulted_tokens_price >= lower_limit && resulted_tokens_price <= upper_limit
}

pub fn address_to_str(address: Address) -> String {
    format!("0x{:x}", address)
}

pub fn get_transfer_payload_params(
    hex_data: HexString,
) -> Result<TxTransferParams, Box<dyn StdError>> {
    let data = hex_data.to_vec();
    if data.is_err() {
        return Err(format!("{:?}", data.err()).into());
    }
    let decoded_data = decode(
        &[
            ParamType::Address,                            //to
            ParamType::Array(Box::new(ParamType::String)), //tokens
            ParamType::Array(Box::new(ParamType::String)), //amounts
            ParamType::Uint(256),                          //amount
            ParamType::Uint(256), //slippage in dollar, which is also * 10^9
            ParamType::String,    // refund_token
        ],
        &data.unwrap(),
    );
    if decoded_data.is_err() {
        return Err(format!("{:?}", decoded_data.err()).into());
    }
    let decoded_data = decoded_data.unwrap();

    // to
    let res = decoded_data[0].clone().into_address();
    if res.is_none() {
        return Err("to is none".into());
    }
    let to = res.unwrap();

    // tokens
    let res = decoded_data[1].clone().into_array();
    if res.is_none() {
        return Err("tokens is none".into());
    }
    let tokens = res
        .unwrap()
        .into_iter()
        .filter_map(|t| {
            let res = t.into_string();
            if res.is_none() {
                return None;
            }
            Some(res.unwrap())
        })
        .collect::<Vec<String>>();

    // data
    let res = decoded_data[2].clone().into_array();
    if res.is_none() {
        return Err("data is none".into());
    }
    let data = res
        .unwrap()
        .into_iter()
        .filter_map(|t| {
            let res = t.into_string();
            if res.is_none() {
                return None;
            }
            Some(HexString::new(res.unwrap()))
        })
        .collect::<Vec<HexString>>();

    // amount
    let res = decoded_data[3].clone().into_uint();
    if res.is_none() {
        return Err("amount is none".into());
    }
    let amount = Uint128::from(res.unwrap().as_u128());

    // slippage
    let res = decoded_data[4].clone().into_uint();
    if res.is_none() {
        return Err("slippage is none".into());
    }
    let slippage = Uint128::from(res.unwrap().as_u128());

    // refund_token
    let res = decoded_data[5].clone().into_string();
    if res.is_none() {
        return Err("refund_token is none".into());
    }
    let refund_token = res.unwrap();

    Ok(TxTransferParams {
        data,
        refund_token,
        to: Address::from_slice(to.as_bytes()),
        tokens,
        amount,
        slippage,
    })
}

pub fn get_crosschain_transfer_payload_params(
    hex_data: HexString,
) -> Result<TxCrossChainTransferParams, Box<dyn StdError>> {
    let data = hex_data.to_vec();
    if data.is_err() {
        return Err(format!("{:?}", data.err()).into());
    }
    let decoded_data = decode(
        &[
            ParamType::Address,                            //to
            ParamType::Array(Box::new(ParamType::String)), //tokens
            ParamType::Array(Box::new(ParamType::Bytes)),  //data
        ],
        &data.unwrap(),
    );
    if decoded_data.is_err() {
        return Err(format!("{:?}", decoded_data.err()).into());
    }
    let decoded_data = decoded_data.unwrap();

    // to
    let res = decoded_data[0].clone().into_address();
    if res.is_none() {
        return Err("to is none".into());
    }
    let to = res.unwrap();

    // tokens
    let res = decoded_data[1].clone().into_array();
    if res.is_none() {
        return Err("tokens is none".into());
    }
    let tokens = res
        .unwrap()
        .into_iter()
        .filter_map(|t| {
            let res = t.into_string();
            if res.is_none() {
                return None;
            }
            Some(res.unwrap())
        })
        .collect::<Vec<String>>();

    // data
    let res = decoded_data[2].clone().into_array();
    if res.is_none() {
        return Err("data is none".into());
    }
    let data = res
        .unwrap()
        .into_iter()
        .filter_map(|t| {
            let res = t.into_bytes();
            if res.is_none() {
                return None;
            }
            Some(HexString::from_vec(res.unwrap()))
        })
        .collect::<Vec<HexString>>();

    Ok(TxCrossChainTransferParams {
        data,
        recipient: Address::from_slice(to.as_bytes()),
        tokens,
    })
}

pub fn decode_crosschain_request_type_data(
    data: &HexString,
) -> Result<CrossChainRequestTypeData, Box<dyn std::error::Error>> {
    let data = data.to_vec();
    if data.is_err() {
        return Err(format!("{:?}", data.err()).into());
    }
    let decoded_data = decode(
        &[
            ParamType::Uint(8),   //request type
            ParamType::String,    //src_chain_id
            ParamType::Uint(256), //src_nonce
            ParamType::String,    //dst_chain_id
            ParamType::Uint(256), //dst_nonce
            ParamType::Address,   //validator
        ],
        &data.unwrap(),
    );
    if decoded_data.is_err() {
        return Err(format!("{:?}", decoded_data.err()).into());
    }
    let decoded_data = decoded_data.unwrap();

    //request_type
    let res = decoded_data[0].clone().into_uint();
    if res.is_none() {
        return Err("request_type is none".into());
    }
    let request_type = res.unwrap().as_u32() as u8;

    //src_chain_id
    let res = decoded_data[1].clone().into_string();
    if res.is_none() {
        return Err("src_chain_id is none".into());
    }
    let src_chain_id = res.unwrap();

    //src_nonce
    let res = decoded_data[2].clone().into_uint();
    if res.is_none() {
        return Err("src_nonce is none".into());
    }
    let src_nonce = Uint128::from(res.unwrap().as_u128());

    //dst_chain_id
    let res = decoded_data[3].clone().into_string();
    if res.is_none() {
        return Err("dst_chain_id is none".into());
    }
    let dst_chain_id = res.unwrap();

    //dst_nonce
    let res = decoded_data[4].clone().into_uint();
    if res.is_none() {
        return Err("dst_nonce is none".into());
    }
    let dst_nonce = Uint128::from(res.unwrap().as_u128());

    //validator
    let res = decoded_data[5].clone().into_address();
    if res.is_none() {
        return Err("validator is none".into());
    }
    let validator = Address::from_slice(res.unwrap().as_bytes());
    Ok(CrossChainRequestTypeData {
        request_type,
        src_chain_id,
        src_nonce,
        dst_chain_id,
        dst_nonce,
        validator,
    })
}

pub fn encode_crosschain_request_type_data(
    request_type: u8,
    src_chain_id: String,
    src_nonce: Uint128,
    dst_chain_id: String,
    dst_nonce: Uint128,
    validator: Address,
) -> HexString {
    let request_type_token: Token = Token::Uint(U256::from(request_type));
    let src_chain_id_token = Token::String(src_chain_id);
    let src_nonce_token = Token::Uint(U256::from(src_nonce.u128()));
    let dst_chain_id_token = Token::String(dst_chain_id);
    let dst_nonce_token = Token::Uint(U256::from(dst_nonce.u128()));
    let validator_token = Token::Address(EthRouterAddress::from_slice(validator.as_bytes()));
    let edata = encode(&vec![
        request_type_token,
        src_chain_id_token,
        src_nonce_token,
        dst_chain_id_token,
        dst_nonce_token,
        validator_token,
    ]);
    HexString::from_vec(edata)
}

pub fn get_crosschain_withdraw_payload(
    request_type: u8,
    tokens: Vec<Address>,
    amounts: Vec<Uint128>,
    sender: Address,
    recipient: Address,
    message: Vec<u8>,
) -> HexString {
    let request_type_token: Token = Token::Uint(U256::from(request_type));
    let tokens_token = Token::Array(
        tokens
            .into_iter()
            .map(|t| Token::Address(EthRouterAddress::from_slice(t.as_bytes())))
            .collect(),
    );
    let amounts_token = Token::Array(
        amounts
            .into_iter()
            .map(|t| Token::Uint(U256::from(t.u128())))
            .collect(),
    );
    let sender_token = Token::Address(EthRouterAddress::from_slice(sender.as_bytes()));
    let recipient_token = Token::Address(EthRouterAddress::from_slice(recipient.as_bytes()));
    let message_token = Token::Bytes(message);
    let edata = encode(&vec![
        request_type_token,
        tokens_token,
        amounts_token,
        sender_token,
        recipient_token,
        message_token,
    ]);
    HexString::from_vec(edata)
}

pub fn abs(a: Uint128, b: Uint128) -> Uint128 {
    if a > b {
        a - b
    } else {
        b - a
    }
}

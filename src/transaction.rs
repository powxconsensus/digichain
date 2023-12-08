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

impl Transaction {
    pub fn new(
        &self,
        hash: String,
        chain_id: String,
        created_at: u64,
        nonce: Uint128,
        from: Address,
        data: HexString,
        signature: Signature,
        tx_type: TxType,
    ) -> Transaction {
        Transaction {
            tx_type,
            chain_id,
            hash,
            nonce,
            created_at,
            timestamp: 0u64,
            from,
            data,
            signature,
            block_number: 0u64,
            result: TxExecutionResult::None,
        }
    }

    pub fn calculate_hash(&self) -> String {
        let serialized_tx = serde_json::to_string(self).expect("Serialization failed");
        format!("0x{}", sha256::digest(serialized_tx))
    }

    pub fn get_raw_tx(&self) -> RawTransaction {
        RawTransaction {
            chain_id: self.chain_id.clone(),
            hash: self.hash.clone(),
            nonce: self.nonce,
            created_at: self.created_at,
            from: self.from,
            data: self.data.clone(),
            signature: self.signature,
            tx_type: self.tx_type.clone(),
        }
    }

    pub fn is_valid(&self, digichain: DigiChain) -> bool {
        //Required: data and to tokens length  same
        // if self.data.len() != self.tokens.len() {
        //     return false;
        // }
        // // refund token should be be present in tokens list
        // if !tokens.contains(&refund_token) {
        //     return false;
        // }
        // let from_account = accounts.get(&self.from).unwrap();
        // if from_account.non
        // if tx not signed for this chain
        if self.chain_id != digichain.chain_id {
            return false;
        }
        //TODO: is this tx is signed by from user or not
        // let msg_bytes = self.get_raw_tx().to_byte();
        // if msg_bytes.is_err() {
        //     return false;
        // }
        // let msg_bytes = msg_bytes.unwrap();
        // let res = self.signature.recover(msg_bytes);
        // if res.is_err() {
        //     return false;
        // }
        // let original_signer = res.unwrap();
        // if original_signer != self.from {
        //     return false;
        // }

        // let res = ethers::abi::Hash::from_slice(&msg_bytes);
        // if ethers::abi::Hash::from_slice(&msg_bytes) {
        // }
        true
    }

    pub fn execute(
        &mut self,
        block_number: u64,
        timestamp: u64,
        digichain: &mut DigiChain,
    ) -> Result<Vec<u8>, Box<dyn StdError>> {
        // before executing
        match &self.tx_type {
            TxType::Transfer => self.transfer_token(block_number, timestamp, digichain),
            TxType::CrosschainTransfer(dst_chain_id) => self.crosschain_transfer_token(
                dst_chain_id.clone(),
                block_number,
                timestamp,
                digichain,
            ),
            TxType::UserKYC => self.user_kyc(block_number, timestamp, digichain),
            TxType::CrossChainRequest(data) => {
                self.add_cross_chain_request(&data.clone(), block_number, timestamp, digichain)
            }
            TxType::AddToken => self.add_token(block_number, timestamp, digichain),
            TxType::AddContractConfig => {
                self.add_contract_config(block_number, timestamp, digichain)
            }
            TxType::UpdateTokenAccepts => {
                self.update_token_accepts(block_number, timestamp, digichain)
            }
            TxType::UpdateTokensPrice => {
                self.update_tokens_price(block_number, timestamp, digichain)
            }
            _ => Err("only user kyc exist as of now".into()),
        }
    }

    fn update_token_accepts(
        &mut self,
        block_number: u64,
        timestamp: u64,
        digichain: &mut DigiChain,
    ) -> Result<Vec<u8>, Box<dyn StdError>> {
        let mut accounts = digichain.accounts.write().unwrap();
        let params = get_update_accepts_payload(self.data.clone());
        if params.is_err() {
            return Err(format!("decoding data: {:?}", params.err()).into());
        }
        let params = params.unwrap();
        let tokens = params.tokens.clone();
        let amounts = params.amounts.clone();
        if tokens.len() != amounts.len() {
            return Err(format!("amounts and tokens length !=").into());
        }
        let token_bindings = digichain.token_list.read().unwrap();
        let res = accounts.get_mut(&self.from);
        if res.is_none() {
            return Err(format!("account not found").into());
        }
        let mut account = res.unwrap().write().unwrap();
        for idx in 0..tokens.len() {
            let res = token_bindings.get(&tokens[idx]);
            if res.is_none() {
                return Err(format!("{} : token not found", tokens[idx]).into());
            }
        }
        let res = account.update_accepts(params);
        if res.is_err() {
            return Err(format!("{:?}", res.err()).into());
        }
        Ok(vec![])
    }

    fn transfer_token(
        &mut self,
        block_number: u64,
        timestamp: u64,
        digichain: &mut DigiChain,
    ) -> Result<Vec<u8>, Box<dyn StdError>> {
        let mut token_list = digichain.token_list.write().unwrap();
        let params = get_transfer_payload_params(self.data.clone());
        if params.is_err() {
            return Err(format!("decoding data: {:?}", params.err()).into());
        }
        let params: crate::types::TxTransferParams = params.unwrap();
        let mut resulted_tokens_price: u128 = 0u128;
        let tokens = params.tokens;
        let data = params.data;
        if tokens.len() != data.len() {
            return Err(format!("tokens and data length mismatch").into());
        }
        let mut result: String = String::new();
        for idx in 0..tokens.len() {
            // check if token exist
            let to_token = tokens[idx].clone();
            let tres = token_list.get_mut(&to_token);
            if tres.is_none() {
                return Err(format!("token not found {:?}", to_token).into());
            }
            let data = data[idx].to_vec();
            if data.is_err() {
                return Err(format!("{:?}", data.err()).into());
            }
            let data = data.unwrap();
            // token exist
            let ttoken: &mut DigiToken = tres.unwrap();
            let res = ttoken.execute(self.tx_type.clone(), self.from.clone(), data, None);
            if res.is_err() {
                return Err(
                    format!("{:?}", res.err().unwrap_or_else(|| "UnknownError".into())).into(),
                );
            }
            let tx_execution_result = res.unwrap();
            let res = decode(&[ParamType::Uint(256)], &tx_execution_result);
            if res.is_err() {
                return Err(format!(
                    "{:?}",
                    res.err()
                        .unwrap_or_else(|| EthError::Other("UnknownError".into()))
                )
                .into());
            }
            let res = res.unwrap();
            resulted_tokens_price =
                resulted_tokens_price + res[0].clone().into_uint().unwrap().as_u128();

            // just for simple logging
            if idx == 0usize {
                result.push_str(&format!("Response: {:?}", tx_execution_result));
            } else {
                result.push_str(&format!("{:?} || {:?}", result, tx_execution_result));
            }
        }
        if !is_within_slippage(
            params.amount,
            params.slippage,
            Uint128::from(resulted_tokens_price),
        ) {
            return Err(format!(
                "Error: resulted value is not in slippage range | Resulted_Amount:: {:?}",
                resulted_tokens_price
            )
            .into());
        }

        Ok(result.into_bytes().to_vec())
    }

    fn crosschain_transfer_token(
        &mut self,
        dst_chain_id: String,
        block_number: u64,
        timestamp: u64,
        digichain: &mut DigiChain,
    ) -> Result<Vec<u8>, Box<dyn StdError>> {
        let mut token_list = digichain.token_list.write().unwrap();
        let mut mempool = digichain.mempool.write().unwrap();

        let mut dst_tokens: Vec<Address> = Vec::new();
        let mut dst_amounts: Vec<Uint128> = Vec::new();
        let params = get_crosschain_transfer_payload_params(self.data.clone());
        if params.is_err() {
            return Err(format!("decoding data: {:?}", params.err()).into());
        }
        let params: crate::types::TxCrossChainTransferParams = params.unwrap();

        let tokens = params.tokens;
        let data = params.data;
        for idx in 0..tokens.len() {
            // check if token exist
            let to_token = tokens[idx].clone();
            let tres = token_list.get_mut(&to_token);
            if tres.is_none() {
                return Err(format!("token not found {:?}", to_token).into());
            }
            let data = data[idx].to_vec();
            if data.is_err() {
                return Err(format!("{:?}", data.err()).into());
            }
            let data = data.unwrap();
            // token exist
            let ttoken: &mut DigiToken = tres.unwrap();

            let res = ttoken.execute(
                self.tx_type.clone(),
                self.from.clone(),
                data.clone(),
                Some(&mut digichain.crosschain.write().unwrap()),
            );
            if res.is_err() {
                return Err(
                    format!("{:?}", res.err().unwrap_or_else(|| "UnknownError".into())).into(),
                );
            }
            let tx_execution_result = res.unwrap();
            let res = decode(&vec![ParamType::Address], &tx_execution_result);
            if res.is_err() {
                return Err(format!(
                    "{:?}",
                    res.err()
                        .unwrap_or_else(|| EthError::Other("UnknownError".into()))
                )
                .into());
            }
            let res = res.unwrap();
            let res = res[0].clone().into_address();
            if res.is_none() {
                return Err(format!("UnknownError").into());
            }

            dst_tokens.push(Address::from_slice(&res.unwrap().as_bytes().to_vec()));
            let res = decode_crosschain_tx_data(data);
            dst_amounts.push(res.unwrap());
        }

        let mut crosschain = digichain.crosschain.write().unwrap();
        let src_nonce = crosschain.increase_nonce();
        let udata = digichain.get_cmp_ccr_data(
            &mut crosschain,
            UnLockedWithdrawRequest,
            digichain.chain_id.clone(),
            dst_chain_id.clone(),
            src_nonce,
            Uint128::from(0u128),
        );

        let proposal = Proposal::new(
            digichain.chain_id.clone(),
            ProposalType::CrossChainRequest(udata.clone()),
            self.from,
            timestamp,
            HexString::from_vec(vec![]),
            self.nonce,
            block_number,
            Some(ExtraData::WithdrawData(CrossChainWithdrawMsg {
                dst_chain_id,
                src_chain_id: digichain.chain_id.clone(),
                src_nonce,
                payload: get_crosschain_withdraw_payload(
                    UnLockedWithdrawRequest,
                    dst_tokens,
                    dst_amounts,
                    self.from,
                    params.recipient,
                    vec![],
                ),
                sigs: vec![],
            })),
        );
        let _ = mempool.add_proposal(ProposalType::CrossChainRequest(udata), proposal);
        Ok(vec![])
    }

    fn user_kyc(
        &mut self,
        block_number: u64,
        timestamp: u64,
        digichain: &mut DigiChain,
    ) -> Result<Vec<u8>, Box<dyn StdError>> {
        let mut mempool = digichain.mempool.write().unwrap();
        // is kyc data valid? for now yes
        let proposal = Proposal::new(
            digichain.chain_id.clone(),
            ProposalType::UserKYC,
            self.from,
            timestamp,
            self.data.clone(),
            self.nonce,
            block_number,
            None,
        );
        let _ = mempool.add_proposal(ProposalType::UserKYC, proposal);
        Ok(vec![])
    }

    fn add_cross_chain_request(
        &mut self,
        data: &HexString,
        block_number: u64,
        timestamp: u64,
        digichain: &mut DigiChain,
    ) -> Result<Vec<u8>, Box<dyn StdError>> {
        let mut mempool = digichain.mempool.write().unwrap();
        let mut crosschain = digichain.crosschain.write().unwrap();

        //TODO: is crosschain request data valid? for now yes
        let info = decode_crosschain_request_type_data(&data);
        if info.is_err() {
            return Err(format!("{:?}", info.err()).into());
        }
        let info = info.unwrap();
        let udata = digichain.get_cmp_ccr_data(
            &mut crosschain,
            info.request_type,
            info.src_chain_id,
            info.dst_chain_id.clone(),
            info.src_nonce,
            info.dst_nonce,
        );
        let proposal = Proposal::new(
            digichain.chain_id.clone(),
            ProposalType::CrossChainRequest(udata.clone()),
            self.from.clone(),
            timestamp,
            self.data.clone(),
            self.nonce,
            block_number,
            None,
        );
        let _ = mempool.add_proposal(ProposalType::CrossChainRequest(udata), proposal);
        Ok(vec![])
    }

    fn add_token(
        &mut self,
        block_number: u64,
        timestamp: u64,
        digichain: &mut DigiChain,
    ) -> Result<Vec<u8>, Box<dyn StdError>> {
        let mut mempool = digichain.mempool.write().unwrap();
        //TODO: is add token data valid? for now yes
        let proposal = Proposal::new(
            digichain.chain_id.clone(),
            ProposalType::AddToken,
            self.from,
            timestamp,
            self.data.clone(),
            self.nonce,
            block_number,
            None,
        );
        let _ = mempool.add_proposal(ProposalType::AddToken, proposal);
        Ok(vec![])
    }

    fn add_contract_config(
        &mut self,
        block_number: u64,
        timestamp: u64,
        digichain: &mut DigiChain,
    ) -> Result<Vec<u8>, Box<dyn StdError>> {
        let mut mempool = digichain.mempool.write().unwrap();
        //TODO: is add_contract_config data valid? for now yes
        let proposal = Proposal::new(
            digichain.chain_id.clone(),
            ProposalType::AddContractConfig,
            self.from,
            timestamp,
            self.data.clone(),
            self.nonce,
            block_number,
            None,
        );
        let _ = mempool.add_proposal(ProposalType::AddContractConfig, proposal);
        Ok(vec![])
    }

    fn update_tokens_price(
        &mut self,
        block_number: u64,
        timestamp: u64,
        digichain: &mut DigiChain,
    ) -> Result<Vec<u8>, Box<dyn StdError>> {
        let mut mempool = digichain.mempool.write().unwrap();
        //TODO: is add_contract_config data valid? for now yes
        let proposal = Proposal::new(
            digichain.chain_id.clone(),
            ProposalType::UpdateTokensPrice,
            self.from,
            timestamp,
            self.data.clone(),
            self.nonce,
            block_number,
            None,
        );
        let _ = mempool.add_proposal(ProposalType::UpdateTokensPrice, proposal);
        Ok(vec![])
    }
}

fn get_update_accepts_payload(
    hex_data: HexString,
) -> Result<TokenAcceptsParams, Box<dyn StdError>> {
    let data = hex_data.to_vec();
    if data.is_err() {
        return Err(format!("{:?}", data.err()).into());
    }
    let decoded_data = decode(
        &[
            ParamType::Array(Box::new(ParamType::String)), // tokens
            ParamType::Array(Box::new(ParamType::Uint(256))), // amounts
        ],
        &data.unwrap(),
    );
    if decoded_data.is_err() {
        return Err(format!("{:?}", decoded_data.err()).into());
    }
    let decoded_data = decoded_data.unwrap();

    //tokens
    let res = decoded_data[0].clone().into_array();
    if res.is_none() {
        return Err("tokens is none".into());
    }
    let tokens = res
        .unwrap()
        .into_iter()
        .map(|t| t.into_string().unwrap())
        .collect::<Vec<String>>();

    //amounts
    let res = decoded_data[1].clone().into_array();
    if res.is_none() {
        return Err("amounts is none".into());
    }
    let amounts = res
        .unwrap()
        .into_iter()
        .map(|t| Uint128::from(t.into_uint().unwrap().as_u128()))
        .collect::<Vec<Uint128>>();

    Ok(TokenAcceptsParams { tokens, amounts })
}

impl RawTransaction {
    pub fn to_byte(&self) -> Result<Vec<u8>, serde_json::Error> {
        serde_json::to_vec(self)
    }
}

impl<'de> Deserialize<'de> for TxType {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        use serde::de::{MapAccess, Visitor};
        struct TxTypeVisitor;
        impl<'de> Visitor<'de> for TxTypeVisitor {
            type Value = TxType;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("an enum variant")
            }

            fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                let parsed = parse_tx_type(value);
                match parsed {
                    Some(tx_type) => Ok(tx_type),
                    None => Err(serde::de::Error::custom("Invalid TxType format")),
                }
            }
        }
        deserializer.deserialize_str(TxTypeVisitor)
    }
}

fn parse_tx_type(value: &str) -> Option<TxType> {
    let mut start: usize = 0usize;
    let mut variant = value;
    let mut additional_info = "";
    if let Some(s) = value.find('(') {
        start = s;
        variant = &value[..s];
    }
    if let Some(e) = value.find(')') {
        additional_info = &value[start + 1..e];
    }
    match variant {
        "Transfer" => Some(TxType::Transfer),
        "CrosschainTransfer" => Some(TxType::CrosschainTransfer(additional_info.to_string())),
        "CrossChainRequest" => Some(TxType::CrossChainRequest(HexString::from_str(
            additional_info,
        ))),
        "UserKYC" => Some(TxType::UserKYC),
        "None" => Some(TxType::None),
        "AddContractConfig" => Some(TxType::AddContractConfig),
        "AddToken" => Some(TxType::AddToken),
        "UpdateTokenAccepts" => Some(TxType::UpdateTokenAccepts),
        "UpdateTokensPrice" => Some(TxType::UpdateTokensPrice),
        _ => None,
    }
}

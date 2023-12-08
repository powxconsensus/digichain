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

impl Proposal {
    pub fn new(
        chain_id: String,
        proposal_type: ProposalType,
        proposed_by: Address,
        proposed_at: u64,
        data: HexString,
        nonce: Uint128,
        block_number: u64,
        extra_data: Option<ExtraData>,
    ) -> Proposal {
        let mut proposal = Proposal {
            hash: Default::default(),
            chain_id,
            proposal_type,
            proposed_by,
            proposed_at,
            data,
            nonce,
            block_number,
            signature: Signature {
                r: Default::default(),
                s: Default::default(),
                v: Default::default(),
            },
            validtors_signature: vec![],
            timestamp: Default::default(),
            result: TxExecutionResult::None,
            extra_data,
        };
        proposal.hash = proposal.calculate_hash();
        proposal
    }

    pub fn calculate_hash(&self) -> String {
        let serialized_tx =
            serde_json::to_string(&self.get_proposal_make()).expect("Serialization failed");
        format!("0x{}", sha256::digest(serialized_tx))
    }

    pub fn is_valid(&self, digichain: DigiChain) -> bool {
        let accounts = digichain.accounts.read().unwrap();
        // if tx not signed for this chain
        if self.chain_id != digichain.chain_id {
            return false;
        }
        let binding = digichain.validators.read().unwrap();
        let votes = self.validtors_signature.len();
        if votes * 100 < 70 * binding.len() {
            //< 70% majority
            return false;
        }

        // self.validtors_signature.len() >
        // TODO: uncomment
        // is this tx is signed by from user or not
        // let msg_bytes = self.get_raw_proposal().to_byte();
        // if msg_bytes.is_err() {
        //     return false;
        // }
        // let msg_bytes = msg_bytes.unwrap();
        // let res = self.signature.recover(msg_bytes);
        // if res.is_err() {
        //     return false;
        // }
        // let original_signer = res.unwrap();
        // if original_signer != self.proposed_by {
        //     return false;
        // }
        true
    }

    pub fn is_signed(&self, address: Address) -> bool {
        let res = self.validtors_signature.clone().into_iter().find(|kv| {
            if kv.0 == address {
                return true;
            }
            false
        });
        res.is_some()
    }

    pub fn get_raw_proposal(&self) -> RawProposal {
        RawProposal {
            signature: self.signature,
            hash: self.hash.clone(),
            chain_id: self.chain_id.clone(),
            proposal_type: self.proposal_type.clone(),
            proposed_by: self.proposed_by,
            proposed_at: self.proposed_at,
            data: self.data.clone(),
            nonce: self.nonce,
            extra_data: self.extra_data.clone(),
        }
    }

    pub fn get_proposal_make(&self) -> ProposalHashMake {
        ProposalHashMake {
            chain_id: self.chain_id.clone(),
            proposal_type: self.proposal_type.clone(),
            data: self.data.clone(),
        }
    }

    pub fn execute(
        &mut self,
        block_number: u64,
        timestamp: u64,
        digichain: &mut DigiChain,
    ) -> Result<Vec<u8>, Box<dyn StdError>> {
        // before executing
        match &self.proposal_type {
            ProposalType::UserKYC => self.user_kyc(timestamp, digichain),
            ProposalType::AddToken => self.add_token(timestamp, digichain),
            ProposalType::AddContractConfig => self.add_contract_config(timestamp, digichain),
            ProposalType::CrossChainRequest(data) => {
                self.add_crosschain_request(timestamp, data.clone(), digichain)
            }
            ProposalType::UpdateTokensPrice => self.update_tokens_price(timestamp, digichain),
            _ => Err("proposal type not exist".into()),
        }
    }

    fn update_tokens_price(
        &mut self,
        timestamp: u64,
        digichain: &mut DigiChain,
    ) -> Result<Vec<u8>, Box<dyn StdError>> {
        let params = get_update_tokens_payload(self.data.clone());
        if params.is_err() {
            return Err(format!("decoding data: {:?}", params.err()).into());
        }
        let params: UpdateTokensPriceParams = params.unwrap();
        let mut binding = digichain.token_list.write().unwrap();

        for idx in 0..params.tokens.len() {
            let res = binding.get_mut(params.tokens.get(idx).unwrap());
            if res.is_none() {
                return Err(format!("some token not found").into());
            }
            let token = res.unwrap();
            let pprice = Uint128::from(token.price);
            token.update_token_price(params.prices[idx].clone());
            println!(
                "Updated Token Price || TokenId: {}, PriceFrom: {} * 10^-9, PriceTo: {} * 10^-9",
                token.id,
                pprice,
                params.prices[idx].clone()
            );
        }
        Ok(vec![])
    }

    fn user_kyc(
        &mut self,
        timestamp: u64,
        digichain: &mut DigiChain,
    ) -> Result<Vec<u8>, Box<dyn StdError>> {
        let params = get_kyc_data(self.data.clone());
        if params.is_err() {
            return Err(format!("decoding data: {:?}", params.err()).into());
        }
        let params = params.unwrap();
        let mut accounts = digichain.accounts.write().unwrap();
        let account;
        if let Some(account_ref) = accounts.get_mut(&self.proposed_by) {
            account = account_ref;
        } else {
            let new_account = Arc::new(RwLock::new(Account::new(self.proposed_by)));
            accounts.insert(self.proposed_by, new_account);
            account = accounts.get_mut(&self.proposed_by).unwrap();
        }
        account.write().unwrap().do_kyc(timestamp, params);
        Ok(vec![])
    }

    fn add_token(
        &mut self,
        timestamp: u64,
        digichain: &mut DigiChain,
    ) -> Result<Vec<u8>, Box<dyn StdError>> {
        let params = get_add_token_data(self.data.clone());
        if params.is_err() {
            return Err(format!("decoding data: {:?}", params.err()).into());
        }
        let params: AddTokenParams = params.unwrap();
        let mut tokens = digichain.token_list.write().unwrap();
        let token = DigiToken::new(
            params.name.clone(),
            params.symbol.clone(),
            params.decimal,
            params.price,
            params.chain_token_mapping.clone(),
        );
        tokens.insert(token.id.clone(), token.clone());

        let mut chain_id_to_token_mp = digichain.chain_id_to_token_mp.write().unwrap();
        for (k, v) in params.chain_token_mapping {
            chain_id_to_token_mp.insert((k.clone(), v.to_lowercase()), token.id.clone());
        }
        println!(
            "Token Added || Id: {}, Symbol: {}, Name: {}, Price: {}",
            token.id, params.symbol, params.name, params.price
        );
        Ok(vec![])
    }

    fn add_contract_config(
        &mut self,
        timestamp: u64,
        digichain: &mut DigiChain,
    ) -> Result<Vec<u8>, Box<dyn StdError>> {
        let params = get_add_contract_config(self.data.clone());
        if params.is_err() {
            return Err(format!("decoding data: {:?}", params.err()).into());
        }
        let params: AddContractConfigParams = params.unwrap();
        let mut crosschain = digichain.crosschain.write().unwrap();
        let res = crosschain.add_contract_config(
            params.chain_id.clone(),
            params.contract_address.clone(),
            params.start_block.clone(),
            params.chain_type,
        );
        if res.is_err() {
            return Err(format!("add_contract_config: {:?}", res.err()).into());
        }
        println!(
            "Contract Config Added || ChainId: {}, ChainType: {}, ContractAdddress: {}, StartBlock: {}",
            params.chain_id, params.chain_type,params.contract_address, params.start_block
        );
        Ok(vec![])
    }

    fn add_crosschain_request(
        &mut self,
        timestamp: u64,
        data: HexString,
        digichain: &mut DigiChain,
    ) -> Result<Vec<u8>, Box<dyn StdError>> {
        let info = decode_crosschain_request_type_data(&data);
        if info.is_err() {
            return Err(format!("{:?}", info.err()).into());
        }
        let info: crate::types::CrossChainRequestTypeData = info.unwrap();

        // emitted from this chain, and will be executed on dst chain soon by crossweaver
        if info.src_chain_id == digichain.chain_id && info.dst_nonce == Uint128::from(0u128) {
            // update
            let mut crosschain = digichain.crosschain.write().unwrap();
            crosschain.add_request(
                info.src_chain_id,
                info.src_nonce,
                CrossChainExecutionResult {
                    src_tx_hash: self.hash.clone(),
                    dst_tx_hash: None,
                    ack_tx_hash: None,
                },
            );
            let res = match &self.proposal_type {
                ProposalType::CrossChainRequest(data) => {
                    let mut mempool = digichain.mempool.write().unwrap();
                    let decode_data = decode_crosschain_request_type_data(&data.clone());
                    if decode_data.is_err() {
                        return Err(format!("{:?}", decode_data.err()).into());
                    }
                    let res: crate::types::CrossChainRequestTypeData = decode_data.unwrap();
                    let extra_data_res: Result<CrossChainWithdrawMsg, Box<dyn StdError>> =
                        match &self.extra_data {
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
                    // always throw error if it's incoming crosschain request
                    if extra_data_res.is_err() {
                        return Err(format!("{:?}", extra_data_res.err()).into());
                    }
                    mempool.add_crosschain_request(res.validator, &extra_data_res.unwrap());
                    Ok(vec![])
                }
                _ => Err(format!("invalid proposal type").into()),
            };
            return res;
        }

        let mut crosschain = digichain.crosschain.write().unwrap();
        let res = crosschain.get_request(info.src_chain_id.clone(), info.src_nonce);
        // emitted from this chain, and reply came here
        if info.request_type == UnLockedWithdrawRequest {
            if res.is_err() {
                return Err(format!("Invalid Request: {:?}", res.err()).into());
            }
            let mut res = res.unwrap();
            let params = get_crr_unlocked_event_payload_params(self.data.clone());
            if params.is_err() {
                return Err(format!("decoding data: {:?}", params.err()).into());
            }
            let params: TxCrossChainReplyParams = params.unwrap();
            let mut binding = crosschain.requests.write().unwrap();
            res.ack_tx_hash = Some(self.hash.clone());
            res.dst_tx_hash = Some(params.dst_tx_hash.clone());
            binding.insert((info.src_chain_id.clone(), info.src_nonce.to_string()), res);
            return Ok(vec![]);
        }

        // emitted from other chain, means user locked there fund and LockedFund event emitted from there
        if info.request_type == LockedFundRequest {
            let params = get_crr_locked_event_payload_params(self.data.clone());
            if params.is_err() {
                return Err(format!("decoding data: {:?}", params.err()).into());
            }
            let params: TxCrossChainRequestParams = params.unwrap();
            //TODO: if any how this tx fails return fund to src chain
            // recipient is registered?
            // sender is registered
            // dst_chainid == self.chainid
            // this nonce process already or
            // verify proposal got enough votes and votes are valid

            // src_chain_id is registered or not
            // src_contract registered?

            // update_contract_config
            let res = crosschain.update_contract_config(
                params.src_chain_id.clone(),
                params.src_nonce,
                params.src_block_number,
            );
            if res.is_err() {
                return Err(format!("{:?}", res.err()).into());
            }
            let token_idx_mp = digichain.chain_id_to_token_mp.read().unwrap();
            if !crosschain.is_contract_registered(params.src_chain_id.clone(), params.src_contract)
            {
                return Err(format!("contract not registered").into());
            }
            let mut result: String = String::new();
            let mut dst_tokens: Vec<Address> = Vec::new();
            let mut dst_amounts: Vec<Uint128> = Vec::new();
            let tokens = params.tokens;
            let amounts = params.amounts;
            for idx in 0..tokens.len() {
                // tokens[idx] is register if yes then tokenid?
                let res = token_idx_mp.get(&(
                    params.src_chain_id.clone(),
                    address_to_str(tokens[idx]).to_lowercase(),
                ));
                if res.is_none() {
                    dst_tokens.push(tokens[idx]);
                    dst_amounts.push(amounts[idx]);
                    result.push_str(&format!(
                        "[{idx}] {:x} token not registered || ",
                        tokens[idx]
                    ));
                    continue;
                    // this token not registerd, need to refund on src chain
                }
                let token_id = res.unwrap();
                let mut token_list = digichain.token_list.write().unwrap();
                let res = token_list.get_mut(token_id);
                if res.is_none() {
                    dst_tokens.push(tokens[idx]);
                    dst_amounts.push(amounts[idx]);
                    result.push_str(&format!("{:x} token not mapped || ", tokens[idx]));
                    continue;
                }
                let token = res.unwrap();
                token.mint(params.recipient.clone(), amounts[idx]);
                return Ok(vec![]);
            }
            if dst_amounts.len() > 0 {
                //TODO: chain will processes block request after certain days, after accumulating block funds
                // create outbound proposal data
                // let dst_tokens_token = dst_tokens
                //     .into_iter()
                //     .map(|token| Token::Address(EthRouterAddress::from_slice(&token.as_bytes())))
                //     .collect();
                // let dst_amounts_token = dst_amounts
                //     .into_iter()
                //     .map(|amount| {
                //         Token::Uint(router_wasm_bindings::ethabi::ethereum_types::U256::from(
                //             amount,
                //         ))
                //     })
                //     .collect();
                // let dst_tokens_token_arr = Token::Array(dst_tokens_token);
                // let dst_amounts_token_arr = Token::Array(dst_amounts_token);
                // let from_token =
                //     Token::Address(EthRouterAddress::from_slice(&params.depositor.as_bytes()));
                // let recipient_token =
                //     Token::Address(EthRouterAddress::from_slice(&params.recipient.as_bytes()));

                // let msg_token = Token::Bytes(vec![]);
                // let msg = rencode(&[
                //     dst_tokens_token_arr,
                //     dst_amounts_token_arr,
                //     from_token,
                //     recipient_token,
                //     msg_token,
                // ]);
                // let self_nonce = crosschain.self_nonce;
                // // drop(crosschain);
                // let udata = digichain.get_cmp_ccr_data(
                //     &mut crosschain,
                //     UnLockedFailedRequest,
                //     digichain.chain_id.clone(),
                //     params.src_chain_id,
                //     params.src_nonce,
                //     self_nonce,
                // );
                // let proposal = Proposal::new(
                //     digichain.chain_id.clone(),
                //     ProposalType::CrossChainRequest(udata.clone()),
                //     self.proposed_by,
                //     timestamp,
                //     HexString::from_vec(msg),
                //     self.nonce,
                //     self.block_number,
                // );
                // let mut mempool = digichain.mempool.write().unwrap();
                // let _ = mempool.add_proposal(ProposalType::CrossChainRequest(udata), proposal);
            }

            let mut binding = crosschain.requests.write().unwrap();
            binding.insert(
                (info.src_chain_id.clone(), info.src_nonce.to_string()),
                CrossChainExecutionResult {
                    src_tx_hash: params.src_tx_hash,
                    dst_tx_hash: Some(self.hash.clone()),
                    ack_tx_hash: None,
                },
            );
            return Ok(vec![]);
        }

        // reply of failed tx request back to chain
        if info.request_type == UnLockedFailedRequest {
            let mut res = res.unwrap();
            let mut binding = crosschain.requests.write().unwrap();
            res.ack_tx_hash = Some(self.hash.clone());
            res.dst_tx_hash = res.dst_tx_hash;
            binding.insert((info.src_chain_id.clone(), info.src_nonce.to_string()), res);
            return Ok(vec![]);
        }
        // will never reach here
        return Ok(vec![]);
    }
}

impl Default for Proposal {
    fn default() -> Self {
        Self {
            result: TxExecutionResult::None,
            hash: Default::default(),
            chain_id: Default::default(),
            proposal_type: ProposalType::None,
            proposed_by: Default::default(),
            proposed_at: Default::default(),
            data: Default::default(),
            nonce: Default::default(),
            block_number: Default::default(),
            validtors_signature: vec![],
            signature: Signature {
                r: U256::default(),
                s: U256::default(),
                v: 0u64,
            },
            timestamp: Default::default(),
            extra_data: None,
        }
    }
}

impl RawProposal {
    pub fn to_byte(&self) -> Result<Vec<u8>, serde_json::Error> {
        serde_json::to_vec(self)
    }
}

fn get_kyc_data(hex_data: HexString) -> Result<KYCParams, Box<dyn StdError>> {
    let data = hex_data.to_vec();
    if data.is_err() {
        return Err(format!("{:?}", data.err()).into());
    }
    let decoded_data = decode(
        &[
            ParamType::String, //name
            ParamType::String, //aadhar
            ParamType::String, //upi_id
            ParamType::String, //mobile_no
            ParamType::String, //address
            ParamType::String, //country
        ],
        &data.unwrap(),
    );
    if decoded_data.is_err() {
        return Err(format!("{:?}", decoded_data.err()).into());
    }
    let decoded_data = decoded_data.unwrap();

    //name
    let res = decoded_data[0].clone().into_string();
    if res.is_none() {
        return Err("name is none".into());
    }
    let name = res.unwrap();

    //aadhar_no
    let res = decoded_data[1].clone().into_string();
    if res.is_none() {
        return Err("aadhar_no is none".into());
    }
    let aadhar_no = res.unwrap();

    //upi_id
    let res = decoded_data[2].clone().into_string();
    if res.is_none() {
        return Err("upi_id is none".into());
    }
    let upi_id = res.unwrap();

    //mobile
    let res = decoded_data[3].clone().into_string();
    if res.is_none() {
        return Err("mobile is none".into());
    }
    let mobile = res.unwrap();

    //address
    let res = decoded_data[4].clone().into_string();
    if res.is_none() {
        return Err("address is none".into());
    }
    let address = res.unwrap();

    //country
    let res = decoded_data[5].clone().into_string();
    if res.is_none() {
        return Err("country is none".into());
    }
    let country = res.unwrap();

    Ok(KYCParams {
        name,
        upi_id,
        address,
        aadhar_no,
        mobile,
        country,
    })
}

fn get_add_token_data(hex_data: HexString) -> Result<AddTokenParams, Box<dyn StdError>> {
    let data = hex_data.to_vec();
    if data.is_err() {
        return Err(format!("{:?}", data.err()).into());
    }
    let decoded_data = decode(
        &[
            ParamType::String,                             //name
            ParamType::String,                             //symbol
            ParamType::Uint(8),                            //symbol
            ParamType::Uint(256),                          //price is f64 * 10^9
            ParamType::Array(Box::new(ParamType::String)), // vec<string> - array of chain ids
            ParamType::Array(Box::new(ParamType::String)), // vec<string> - array of corrosponding chain token
        ],
        &data.unwrap(),
    );
    if decoded_data.is_err() {
        return Err(format!("{:?}", decoded_data.err()).into());
    }
    let decoded_data = decoded_data.unwrap();

    //name
    let res = decoded_data[0].clone().into_string();
    if res.is_none() {
        return Err("name is none".into());
    }
    let name = res.unwrap();

    //symbol
    let res = decoded_data[1].clone().into_string();
    if res.is_none() {
        return Err("symbol is none".into());
    }
    let symbol = res.unwrap();

    //decimal
    let res = decoded_data[2].clone().into_uint();
    if res.is_none() {
        return Err("decimal is none".into());
    }
    let decimal = res.unwrap().as_u32() as u8;

    //price
    let res = decoded_data[3].clone().into_uint();
    if res.is_none() {
        return Err("price is none".into());
    }
    let price = Uint128::from(res.unwrap().as_u128());

    // vec<chain_ids>
    let res = decoded_data[4].clone().into_array();
    if res.is_none() {
        return Err("vec<chain_ids> is none".into());
    }
    let chain_ids_mp = res
        .unwrap()
        .into_iter()
        .filter_map(|token| token.clone().into_string())
        .collect::<Vec<String>>();

    // vec<token_address>
    let res = decoded_data[5].clone().into_array();
    if res.is_none() {
        return Err("vec<token_address> is none".into());
    }
    let token_address_mp = res
        .unwrap()
        .into_iter()
        .filter_map(|token| token.clone().into_string())
        .collect::<Vec<String>>();

    if token_address_mp.len() != chain_ids_mp.len() {
        return Err("token_address_mp.len() != chain_ids_mp.len() ".into());
    }

    let mut chain_token_mapping = HashMap::new();
    for idx in 0..token_address_mp.len() {
        chain_token_mapping.insert(
            chain_ids_mp.get(idx).unwrap().clone(),
            token_address_mp.get(idx).unwrap().clone(),
        );
    }
    Ok(AddTokenParams {
        name,
        symbol,
        decimal,
        price,
        chain_token_mapping,
    })
}

fn get_add_contract_config(
    hex_data: HexString,
) -> Result<AddContractConfigParams, Box<dyn StdError>> {
    let data = hex_data.to_vec();
    if data.is_err() {
        return Err(format!("{:?}", data.err()).into());
    }
    let decoded_data = decode(
        &[
            ParamType::String,
            ParamType::Uint(8),
            ParamType::String,
            ParamType::Uint(64),
        ],
        &data.unwrap(),
    );
    if decoded_data.is_err() {
        return Err(format!("{:?}", decoded_data.err()).into());
    }
    let decoded_data = decoded_data.unwrap();

    // chain id
    let res = decoded_data[0].clone().into_string();
    if res.is_none() {
        return Err("contract_address is none".into());
    }
    let chain_id = res.unwrap();

    // chain_type
    let res = decoded_data[1].clone().into_uint();
    if res.is_none() {
        return Err("start_block is none".into());
    }
    let chain_type = res.unwrap().as_u32() as u8;

    // contract_address
    let res = decoded_data[2].clone().into_string();
    if res.is_none() {
        return Err("contract_address is none".into());
    }
    let contract_address = res.unwrap();

    // start_block
    let res = decoded_data[3].clone().into_uint();
    if res.is_none() {
        return Err("start_block is none".into());
    }
    let start_block = res.unwrap();

    Ok(AddContractConfigParams {
        contract_address,
        start_block: start_block.as_u64(),
        chain_id,
        chain_type,
    })
}

fn get_crr_locked_event_payload_params(
    hex_data: HexString,
) -> Result<TxCrossChainRequestParams, Box<dyn StdError>> {
    let data = hex_data.to_vec();
    if data.is_err() {
        return Err(format!("{:?}", data.err()).into());
    }
    let decoded_data = decode(
        &[
            ParamType::String,                                //src_chain_id
            ParamType::String,                                //dest_chain_id
            ParamType::Address,                               //src_contract
            ParamType::Address,                               //recipient
            ParamType::Address,                               //depositor
            ParamType::Array(Box::new(ParamType::Address)),   //vec<tokens>
            ParamType::Array(Box::new(ParamType::Uint(256))), //vec<amounts>
            ParamType::Uint(256),                             //src_nonce
            ParamType::Uint(64),                              //src_block_number
            ParamType::String,                                //src_tx_hash
        ],
        &data.unwrap(),
    );
    if decoded_data.is_err() {
        return Err(format!("{:?}", decoded_data.err()).into());
    }
    let decoded_data = decoded_data.unwrap();

    // src_chain_id
    let res = decoded_data[0].clone().into_string();
    if res.is_none() {
        return Err("src_chain_id is none".into());
    }
    let src_chain_id = res.unwrap();

    // dst_chain_id
    let res = decoded_data[1].clone().into_string();
    if res.is_none() {
        return Err("dst_chain_id is none".into());
    }
    let dst_chain_id = res.unwrap();
    // src_contract
    let res = decoded_data[2].clone().into_address();
    if res.is_none() {
        return Err("src_contract is none".into());
    }
    let src_contract = res.unwrap();

    // recipient
    let res = decoded_data[3].clone().into_address();
    if res.is_none() {
        return Err("recipient is none".into());
    }
    let recipient = res.unwrap();

    // depositor
    let res = decoded_data[4].clone().into_address();
    if res.is_none() {
        return Err("depositor is none".into());
    }
    let depositor = res.unwrap();
    //tokens
    let res: Option<Vec<Token>> = decoded_data[5].clone().into_array();
    if res.is_none() {
        return Err("tokens is none".into());
    }
    let tokens = res
        .unwrap()
        .into_iter()
        .clone()
        .map(|t| {
            ethereum_types::Address::from_slice(&t.into_address().unwrap().as_bytes().to_vec())
        })
        .collect::<Vec<Address>>();

    //amounts
    let res = decoded_data[6].clone().into_array();
    if res.is_none() {
        return Err("amounts is none".into());
    }
    let amounts = res
        .unwrap()
        .into_iter()
        .clone()
        .map(|t| Uint128::from(t.into_uint().unwrap().as_u128()))
        .collect::<Vec<Uint128>>();

    // src_nonce
    let res = decoded_data[7].clone().into_uint();
    if res.is_none() {
        return Err("src_nonce is none".into());
    }
    let src_nonce = Uint128::from(res.unwrap().as_u128());

    // src_block_number
    let res = decoded_data[8].clone().into_uint();
    if res.is_none() {
        return Err("src_nonce is none".into());
    }
    let src_block_number = res.unwrap().as_u64();

    // src_tx_hash
    let res = decoded_data[9].clone().into_string();
    if res.is_none() {
        return Err("src_tx_hash is none".into());
    }
    let src_tx_hash = res.unwrap();

    Ok(TxCrossChainRequestParams {
        src_chain_id,
        dst_chain_id,
        depositor: Address::from_slice(&depositor.as_bytes()),
        recipient: Address::from_slice(&recipient.as_bytes()),
        src_contract: Address::from_slice(&src_contract.as_bytes()),
        src_nonce,
        tokens,
        amounts,
        src_block_number,
        src_tx_hash,
    })
}

fn get_crr_unlocked_event_payload_params(
    hex_data: HexString,
) -> Result<TxCrossChainReplyParams, Box<dyn StdError>> {
    let data = hex_data.to_vec();
    if data.is_err() {
        return Err(format!("{:?}", data.err()).into());
    }
    let decoded_data = decode(
        &[
            ParamType::String,                                //src_chain_id
            ParamType::String,                                //dest_chain_id
            ParamType::Address,                               //src_contract
            ParamType::Address,                               //recipient
            ParamType::Address,                               //depositor
            ParamType::Array(Box::new(ParamType::Address)),   //vec<tokens>
            ParamType::Array(Box::new(ParamType::Uint(256))), //vec<amounts>
            ParamType::Uint(256),                             //src_nonce
            ParamType::Uint(256),                             //dst_nonce
            ParamType::Uint(64),                              //dst_block_number
            ParamType::String,                                //dst_tx_hash
        ],
        &data.unwrap(),
    );
    if decoded_data.is_err() {
        return Err(format!("{:?}", decoded_data.err()).into());
    }
    let decoded_data = decoded_data.unwrap();

    // src_chain_id
    let res = decoded_data[0].clone().into_string();
    if res.is_none() {
        return Err("src_chain_id is none".into());
    }
    let src_chain_id = res.unwrap();

    // dst_chain_id
    let res = decoded_data[1].clone().into_string();
    if res.is_none() {
        return Err("dst_chain_id is none".into());
    }
    let dst_chain_id = res.unwrap();
    // src_contract
    let res = decoded_data[2].clone().into_address();
    if res.is_none() {
        return Err("src_contract is none".into());
    }
    let src_contract = res.unwrap();

    // recipient
    let res = decoded_data[3].clone().into_address();
    if res.is_none() {
        return Err("recipient is none".into());
    }
    let recipient = res.unwrap();

    // depositor
    let res = decoded_data[4].clone().into_address();
    if res.is_none() {
        return Err("depositor is none".into());
    }
    let depositor = res.unwrap();
    //tokens
    let res: Option<Vec<Token>> = decoded_data[5].clone().into_array();
    if res.is_none() {
        return Err("tokens is none".into());
    }
    let tokens = res
        .unwrap()
        .into_iter()
        .clone()
        .map(|t| Address::from_slice(&t.into_address().unwrap().as_bytes()))
        .collect::<Vec<Address>>();

    //amounts
    let res = decoded_data[6].clone().into_array();
    if res.is_none() {
        return Err("amounts is none".into());
    }
    let amounts = res
        .unwrap()
        .into_iter()
        .clone()
        .map(|t| Uint128::from(t.into_uint().unwrap().as_u128()))
        .collect::<Vec<Uint128>>();

    // src_nonce
    let res = decoded_data[7].clone().into_uint();
    if res.is_none() {
        return Err("src_nonce is none".into());
    }
    let src_nonce = Uint128::from(res.unwrap().as_u128());

    // dst_nonce
    let res = decoded_data[8].clone().into_uint();
    if res.is_none() {
        return Err("dst_nonce is none".into());
    }
    let dst_nonce = Uint128::from(res.unwrap().as_u128());

    // dst_block_number
    let res = decoded_data[9].clone().into_uint();
    if res.is_none() {
        return Err("dst_block_number is none".into());
    }
    let dst_block_number = res.unwrap().as_u64();

    // dst_tx_hash
    let res = decoded_data[10].clone().into_string();
    if res.is_none() {
        return Err("dst_tx_hash is none".into());
    }
    let dst_tx_hash = res.unwrap();

    Ok(TxCrossChainReplyParams {
        src_chain_id,
        dst_chain_id,
        depositor: Address::from_slice(&depositor.as_bytes()),
        recipient: Address::from_slice(&recipient.as_bytes()),
        src_contract: Address::from_slice(&src_contract.as_bytes()),
        src_nonce,
        tokens,
        amounts,
        dst_block_number,
        dst_tx_hash,
        dst_nonce,
    })
}

fn get_update_tokens_payload(
    hex_data: HexString,
) -> Result<UpdateTokensPriceParams, Box<dyn StdError>> {
    let data = hex_data.to_vec();
    if data.is_err() {
        return Err(format!("{:?}", data.err()).into());
    }
    let decoded_data = decode(
        &[
            ParamType::Array(Box::new(ParamType::String)), //vec<tokens>
            ParamType::Array(Box::new(ParamType::Uint(256))), //vec<amounts>
        ],
        &data.unwrap(),
    );
    if decoded_data.is_err() {
        return Err(format!("{:?}", decoded_data.err()).into());
    }
    let decoded_data = decoded_data.unwrap();

    // tokens
    let res: Option<Vec<Token>> = decoded_data[0].clone().into_array();
    if res.is_none() {
        return Err("tokens is none".into());
    }
    let tokens = res
        .unwrap()
        .into_iter()
        .clone()
        .map(|t| t.into_string().unwrap())
        .collect::<Vec<String>>();

    //prices
    let res = decoded_data[1].clone().into_array();
    if res.is_none() {
        return Err("prices is none".into());
    }
    let prices = res
        .unwrap()
        .into_iter()
        .clone()
        .map(|t| Uint128::from(t.into_uint().unwrap().as_u128()))
        .collect::<Vec<Uint128>>();

    if prices.len() != tokens.len() {
        return Err("invalid array length".into());
    }

    Ok(UpdateTokensPriceParams { tokens, prices })
}

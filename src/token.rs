use crate::{
    crosschain::CrossChain,
    digichain::DigiChain,
    transaction::TxType,
    types::{Address, TokenId, TxExecutionResult},
};
use cosmwasm_std::Uint128;
use nanoid::nanoid;
use router_wasm_bindings::ethabi::{
    decode, encode, ethereum_types::U256, Address as EthRouterAddress, Error as EthError,
    ParamType, Token,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, error::Error as StdError, str::FromStr};

pub const ALPHA_KEY: [char; 62] = [
    '0', '1', '2', '3', '4', '5', '6', '7', '8', '9', 'a', 'b', 'c', 'd', 'e', 'f', 'g', 'h', 'i',
    'j', 'k', 'l', 'm', 'n', 'o', 'p', 'q', 'r', 's', 't', 'u', 'v', 'w', 'x', 'y', 'z', 'A', 'B',
    'C', 'D', 'E', 'F', 'G', 'H', 'I', 'J', 'K', 'L', 'M', 'N', 'O', 'P', 'Q', 'R', 'S', 'T', 'U',
    'V', 'W', 'X', 'Y', 'Z',
];

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct DigiToken {
    pub id: TokenId,
    pub name: String,
    pub symbol: String,
    pub decimal: u8,
    pub chain_token_mapping: HashMap<String, String>, // chain_id : token_address on chain with chainid chain_id
    pub price: Uint128,
    pub balance_mp: HashMap<Address, Uint128>,
}

impl DigiToken {
    pub fn new(
        name: String,
        symbol: String,
        decimal: u8,
        price: Uint128,
        chain_token_mapping: HashMap<String, String>,
    ) -> DigiToken {
        //TODO: considering this will generate unique id
        let id = nanoid!(40, &ALPHA_KEY);
        DigiToken {
            decimal,
            id,
            name,
            price,
            symbol,
            chain_token_mapping,
            balance_mp: HashMap::new(),
        }
    }

    pub fn get_balance_of(&self, address: Address) -> Uint128 {
        let zero = Uint128::zero();
        self.balance_mp
            .get(&address)
            .unwrap_or_else(|| &zero)
            .clone()
    }

    pub fn mint(&mut self, to: Address, amount: Uint128) -> bool {
        let zero = Uint128::zero();
        let pamount = self.balance_mp.get(&to).unwrap_or_else(|| &zero);
        self.balance_mp.insert(to, pamount.clone() + amount);
        true
    }

    pub fn get_token_id(self) -> String {
        return self.id;
    }
    pub fn get_name(self) -> String {
        return self.name;
    }
    pub fn get_symbol(self) -> String {
        return self.symbol;
    }
    pub fn get_chain_token(self, chain_id: String) -> Result<String, Box<dyn StdError>> {
        let res = self.chain_token_mapping.get(&chain_id);
        if res.is_none() {
            return Err("token not found with given chain_id".into());
        }
        return Ok(res.unwrap().clone());
    }
    pub fn add_chain_token(mut self, chain_id: String, token: String) {
        //TODO: will be added after consensus only
        self.chain_token_mapping
            .insert(chain_id, token.to_lowercase());
    }
    pub fn update_token_price(&mut self, price: Uint128) {
        //TODO: will be added after consensus only
        self.price = price;
    }

    fn transfer(&mut self, from: Address, data: Vec<u8>) -> Result<Vec<u8>, Box<dyn StdError>> {
        let res = decode_tx_data(data);
        if res.is_err() {
            return Err(res.err().unwrap());
        }
        let (recipient, amount) = res.unwrap();

        // sender and recipient cann't be same
        if from == recipient {
            return Err(format!("sender and recipient cann't be same",).into());
        }
        //Required: from and to user should be registerd, means they registered already
        // if !accounts.contains_key(&self.from) || !accounts.contains_key(&self.to) {
        //     return false;
        // }

        //update sender and recipient balance
        // decrease sender balance by amount
        let zero = Uint128::zero();
        let mut sender_balance = self.balance_mp.get(&from).unwrap_or_else(|| &zero).clone();
        if sender_balance < amount {
            return Err(format!(
                "[{}]: insufficient fund, balance: {}",
                self.id.clone(),
                sender_balance
            )
            .into());
        }
        sender_balance = sender_balance - amount;
        self.balance_mp.insert(from, sender_balance);

        // increase recipient balance by amount
        let zero = Uint128::zero();
        let mut recipient_balance = self
            .balance_mp
            .get(&recipient)
            .unwrap_or_else(|| &zero)
            .clone();
        recipient_balance = recipient_balance + amount;
        self.balance_mp.insert(recipient, recipient_balance);

        let price_in_dollar_token = Token::Uint(U256::from((amount * self.price).u128()));
        Ok(encode(&vec![price_in_dollar_token]))
    }

    fn cross_chain_transfer(
        &mut self,
        crosschain: &mut CrossChain,
        from: Address,
        data: Vec<u8>,
        dst_chain_id: String,
    ) -> Result<Vec<u8>, Box<dyn StdError>> {
        let res = decode_crosschain_tx_data(data);
        if res.is_err() {
            return Err(res.err().unwrap());
        }
        let amount = res.unwrap();

        // check corrsponding token on dst_chain
        let res = self.chain_token_mapping.get(&dst_chain_id);
        if res.is_none() {
            return Err(format!("no token found on dst chain").into());
        }
        let dst_token_address = res.unwrap();

        //update sender and recipient balance
        // decrease sender balance by amount
        let zero = Uint128::zero();
        let mut sender_balance = self.balance_mp.get(&from).unwrap_or_else(|| &zero).clone();
        if sender_balance < amount {
            return Err(format!(
                "[{}]: insufficient fund, balance: {}",
                self.id.clone(),
                sender_balance
            )
            .into());
        }
        sender_balance = sender_balance - amount;
        self.balance_mp.insert(from, sender_balance);

        let dst_token_address_token =
            Token::Address(EthRouterAddress::from_str(&dst_token_address).unwrap());
        Ok(encode(&vec![dst_token_address_token]))
    }

    pub fn execute(
        &mut self,
        tx_type: TxType,
        from: Address,
        data: Vec<u8>,
        crosschain: Option<&mut CrossChain>,
    ) -> Result<Vec<u8>, Box<dyn StdError>> {
        match tx_type {
            TxType::Transfer => self.transfer(from, data),
            TxType::CrosschainTransfer(dst_chain_id) => {
                self.cross_chain_transfer(crosschain.unwrap(), from, data, dst_chain_id)
            }
            _ => Err("unknown fn_sign call, method not exist".into()),
        }
    }
}

fn decode_tx_data(data: Vec<u8>) -> Result<(Address, Uint128), Box<dyn StdError>> {
    let res = decode(&[ParamType::Address, ParamType::Uint(256)], &data);
    if res.is_err() {
        return Err(format!(
            "{:?}",
            res.err()
                .unwrap_or_else(|| EthError::Other("UnknownError".into()))
        )
        .into());
    }
    let decoded_data = res.unwrap();
    let res = decoded_data[0].clone().into_address();
    if res.is_none() {
        return Err(format!("not able to decode data to recipient adress").into());
    }
    let recipient = Address::from_slice(&res.unwrap().as_bytes());

    let res = decoded_data[1].clone().into_uint();
    if res.is_none() {
        return Err(format!("not able to decode data to transfer amount").into());
    }
    let amount = Uint128::from(res.unwrap().as_u128());
    return Ok((recipient, amount));
}

pub fn decode_crosschain_tx_data(data: Vec<u8>) -> Result<Uint128, Box<dyn StdError>> {
    let res = decode(&[ParamType::Uint(256)], &data);
    if res.is_err() {
        return Err(format!(
            "{:?}",
            res.err()
                .unwrap_or_else(|| EthError::Other("UnknownError".into()))
        )
        .into());
    }
    let decoded_data = res.unwrap();
    let res = decoded_data[0].clone().into_uint();
    if res.is_none() {
        return Err(format!("not able to decode data to fn_sign,recipient and amount").into());
    }
    let amount = Uint128::from(res.unwrap().as_u128());
    return Ok(amount);
}

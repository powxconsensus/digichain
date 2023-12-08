use std::{
    collections::{self, HashMap},
    error::Error,
    sync::{Arc, RwLock},
};
use uuid::Uuid;

use crate::{
    acccount::{self, Account},
    block::DigiBlock,
    mempool::Mempool,
    types::Address,
};
use cosmwasm_std::Uint128;

#[derive(Clone, Debug)]
pub struct DigiCoin {
    pub supply: Uint128,
    pub balance_of: HashMap<Address, Uint128>,
}

impl DigiCoin {
    pub fn new(supply: Uint128) -> DigiCoin {
        DigiCoin {
            supply,
            balance_of: HashMap::new(),
        }
    }
    pub fn transfer(self) {}
}

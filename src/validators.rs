use crate::acccount::Account;
use cosmwasm_std::Uint128;
use ethers::core::k256::ecdsa::SigningKey;
use ethers_signers::Wallet;
use rand::thread_rng;

#[derive(Clone, Debug)]
pub struct Validator {
    pub acccount: Account,
    pub staked: Uint128,
    pub wallet: Wallet<SigningKey>,
}

impl Validator {
    pub fn new(acccount: Account, staked: Uint128, wallet: Wallet<SigningKey>) -> Validator {
        Validator {
            acccount,
            staked,
            wallet,
        }
    }
}

impl Default for Validator {
    fn default() -> Self {
        Self {
            acccount: Default::default(),
            staked: Default::default(),
            wallet: Wallet::new(&mut thread_rng()),
        }
    }
}

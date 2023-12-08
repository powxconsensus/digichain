use base64::Engine;
use cosmwasm_std::Uint128;
use qrcode::{render::unicode, EcLevel, QrCode, Version};
use std::fmt;
use std::{collections::HashMap, fmt::Binary};
use svg::node::element::Image;

use crate::{
    transaction::Transaction,
    types::{Address, KYCParams, TokenAcceptsParams, TokenId},
};
use serde::{Deserialize, Serialize};
use std::error::Error;

#[derive(Serialize, Deserialize)]
pub struct ReceiverQrData {
    pub upi_id: String,
    pub account_address: Address,
    pub name: String,
    pub accepts: HashMap<TokenId, Uint128>,
}

#[derive(Clone)]
pub struct QrCodeWrapper(pub qrcode::QrCode);

impl fmt::Debug for QrCodeWrapper {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self.0.render::<unicode::Dense1x2>().build())
    }
}

#[derive(Clone, Debug)]
pub struct Account {
    pub address: Address,
    pub tx_nonce: Uint128,
    pub proposal_nonce: Uint128,
    pub accepts: HashMap<TokenId, Uint128>, // token_id -> max amount can accepts
    pub transactions: Vec<String>,          // array of tx hash
    pub is_kyc_done: bool,
    pub name: String,
    pub country: String,
    pub mobile: String,
    pub upi_id: String,
    pub aadhar_no: String,
    pub kyc_completed_at: u64,
    pub qr_code: QrCodeWrapper,
}

impl Account {
    pub fn new(address: Address) -> Account {
        let mut account = Account {
            address,
            tx_nonce: Uint128::from(0u128),
            proposal_nonce: Uint128::from(0u128),
            transactions: Vec::new(),
            is_kyc_done: false, // register user via kyc
            name: String::default(),
            country: String::default(),
            mobile: String::default(),
            upi_id: String::default(),
            aadhar_no: String::default(),
            accepts: HashMap::default(),
            kyc_completed_at: 0u64,
            qr_code: QrCodeWrapper(QrCode::new(b"").expect("Failed to create an empty QR code")),
        };
        account.update_qr_code();
        return account;
    }

    pub fn to_receiver_qr_data(&self) -> ReceiverQrData {
        ReceiverQrData {
            upi_id: self.upi_id.clone(),
            account_address: self.address.clone(),
            name: self.name.clone(),
            accepts: self.accepts.clone(),
        }
    }

    pub fn update_qr_code(&mut self) {
        // let json_data = serde_json::to_string(&self.to_receiver_qr_data())
        //     .expect("Failed to serialize data to JSON");
        // self.qr_code = QrCodeWrapper(QrCode::new(json_data).unwrap());
    }

    pub fn add_transaction_hash(&mut self, tx_hash: String) -> bool {
        self.transactions.push(tx_hash);
        true
    }

    pub fn increase_tx_nonce(&mut self) -> Uint128 {
        self.tx_nonce = self.tx_nonce + Uint128::from(1u128);
        return self.tx_nonce;
    }

    pub fn increase_proposal_nonce(&mut self) -> Uint128 {
        self.proposal_nonce = self.proposal_nonce + Uint128::from(1u128);
        return self.proposal_nonce;
    }

    pub fn do_kyc(&mut self, timestamp: u64, params: KYCParams) {
        self.aadhar_no = params.aadhar_no;
        self.name = params.name;
        self.mobile = params.mobile;
        self.upi_id = params.upi_id;
        self.country = params.country;
        self.is_kyc_done = true;
        self.kyc_completed_at = timestamp;
        self.update_qr_code();
    }

    pub fn update_accepts(&mut self, params: TokenAcceptsParams) -> Result<bool, Box<dyn Error>> {
        let tokens = params.tokens;
        let amounts = params.amounts;
        if tokens.len() != amounts.len() {
            return Err(format!("amounts and tokens length !=").into());
        }
        for idx in 0..tokens.len() {
            self.accepts.insert(tokens[idx].clone(), amounts[idx]);
        }
        self.update_qr_code();
        Ok(true)
    }

    pub fn get_address(self) -> Address {
        return self.address;
    }

    pub fn get_tx_nonce(self) -> Uint128 {
        return self.tx_nonce;
    }

    pub fn get_transactions(&self) -> Vec<String> {
        self.transactions
            .iter()
            .cloned()
            .map(|tx| tx.clone())
            .collect()
    }
}

impl Default for Account {
    fn default() -> Self {
        Self {
            accepts: HashMap::default(),
            is_kyc_done: false,
            address: Default::default(),
            tx_nonce: Default::default(),
            proposal_nonce: Default::default(),
            transactions: Default::default(),
            name: String::default(),
            mobile: String::default(),
            country: String::default(),
            upi_id: String::default(),
            aadhar_no: String::default(),
            kyc_completed_at: Default::default(),
            qr_code: QrCodeWrapper(QrCode::new(b"").expect("Failed to create an empty QR code")),
        }
    }
}

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

//Account Struct For Chain

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

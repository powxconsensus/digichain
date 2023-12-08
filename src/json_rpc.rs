use std::collections::HashMap;
use std::sync::{Arc, Mutex, MutexGuard};

use crate::acccount::Account;
use crate::digichain::DigiChain;
use crate::proposal::Proposal;
use crate::token::DigiToken;
use crate::transaction::{Transaction, TxType};
use crate::types::{
    Address, AirDropParams, BroadcastTransactionParams, GetAccountParams, GetBalanceOf,
    GetBalances, GetChainParams, GetConfigParams, GetCrossChainRequestReadyToExecute,
    GetCrossChainRequestsParams, GetOptimalPath, GetProposalsParams, GetTokenByChain,
    GetTokenParams, GetTokensParams, GetTransactionParams, GetTransactionsParams,
    IsBroadcastedParams, PauseAndUnPauseParams, UpdateTokensPriceParams,
};
use crate::utils::{abs, decode_crosschain_request_type_data};
use actix_web::{web, HttpRequest, HttpResponse, Responder};
use jsonrpc_http_server::jsonrpc_core::Value;

use cosmwasm_std::Uint128;
use rand::seq::SliceRandom;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::json;

#[derive(Clone, Debug, JsonSchema)]
pub struct JsonRpc {
    pub id: String,
}

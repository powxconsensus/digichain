use std::collections::HashMap;
use std::sync::{Arc, Mutex, MutexGuard};

use crate::acccount::Account;
use crate::digichain::DigiChain;
use crate::proposal::Proposal;
use crate::token::DigiToken;
use crate::transaction::{TxType, Transaction};
use crate::types::{BroadcastTransactionParams, GetAccountParams, GetChainParams, GetTokenParams, GetTokensParams, GetTokenByChain, GetConfigParams, GetCrossChainRequestsParams, GetProposalsParams, GetBalanceOf, Address, GetTransactionParams, GetBalances, GetCrossChainRequestReadyToExecute, UpdateTokensPriceParams, GetOptimalPath, AirDropParams, PauseAndUnPauseParams, IsBroadcastedParams, GetTransactionsParams};
use crate::utils::{decode_crosschain_request_type_data, abs};
use actix_web::{web, HttpRequest, HttpResponse, Responder};
use jsonrpc_http_server::jsonrpc_core::Value;

use rand::seq::SliceRandom;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::json;
use cosmwasm_std::Uint128;

#[derive(Clone, Debug, JsonSchema)]
pub struct JsonRpc {
    pub id: String,
}

impl JsonRpc {
    pub fn new() -> Self {
        JsonRpc {
            id: "0".to_string(),
        }
    }
     fn get_token_by_id(
        &self,
        digichain: MutexGuard<'_, DigiChain>,
        token_id: String,
    ) -> Result<DigiToken,Box<dyn std::error::Error>> {
        let binding = digichain.token_list.read().unwrap();
        let res =  binding.get(&token_id);
        if res.is_none() {
            return Err(format!("token not found").into());
        }
        return Ok(res.unwrap().clone())
    }

    pub fn get_balance_of(
        self,
        digichain: MutexGuard<'_, DigiChain>,
        request_body: Params,
    ) -> HttpResponse {
        let res: Result<GetBalanceOf , serde_json::Error> = serde_json::from_value(request_body.params);
        if res.is_err() {
            return HttpResponse::BadRequest().json(json!(format!("{:?}",res.err()))).into();
        }
        let params: GetBalanceOf = res.unwrap();

        let token_list = digichain.token_list.read().unwrap();
        let res = token_list.get(&params.token_id);
        if res.is_none() {
            return HttpResponse::NotFound().json(json!({ "error": "token not found","id":self.id }));
        }
        return HttpResponse::Ok().json(json!({ "balance": res.unwrap().get_balance_of(params.address),"id":self.id }));
    }


    pub fn get_balances(
        self,
        digichain: MutexGuard<'_, DigiChain>,
        request_body: Params,
    ) -> HttpResponse {
        let res: Result<GetBalances , serde_json::Error> = serde_json::from_value(request_body.params);
        if res.is_err() {
            return HttpResponse::BadRequest().json(json!(format!("{:?}",res.err()))).into();
        }
        let params: GetBalances = res.unwrap();
        let mut balances = HashMap::new();
        if params.tokens.len() != params.addresses.len() {
            return HttpResponse::BadRequest().json(json!({ "error": "length not equal","id":self.id }));
        }
        for idx in 0..params.tokens.len() {
            let token_list = digichain.token_list.read().unwrap();
            let res = token_list.get(&params.tokens[idx].clone());
            if res.is_none() {
                return HttpResponse::NotFound().json(json!({ "error": "token not found","id":self.id }));
            }
            balances.insert(params.tokens[idx].clone(), res.unwrap().get_balance_of(params.addresses[idx].clone()));
        }
        return HttpResponse::Ok().json(json!({ "balance": balances,"id":self.id }));
    }

    pub fn get_block_number(
        self,
        digichain: MutexGuard<'_, DigiChain>,
        request_body: Params,
    ) -> HttpResponse {
        let block_number = digichain.get_block_number();
        return HttpResponse::Ok().json(json!({ "block_number": block_number,"id":self.id }));
    }

    pub fn broadcast_transaction(
        self,
        digichain: MutexGuard<'_, DigiChain>,
        request_body: Params,
    ) -> HttpResponse {
        let res: Result<BroadcastTransactionParams, serde_json::Error> =
            serde_json::from_value(request_body.params.clone());
        if res.is_err() {
            return HttpResponse::BadRequest()
                .json(json!({ "err": format!("{:?}", res.err()) }))
                .into();
        }
        let params: BroadcastTransactionParams = res.unwrap();
        let mut tx = params.transaction.clone().to_transaction();
        // TODO: add validation like is tc.from is validator or not?
        if tx.tx_type != TxType::UserKYC {
            let res = digichain.get_account(tx.from);
            if res.is_err() {
                return HttpResponse::BadRequest()
                .json(json!({ "err": format!("{:?}", res.err()) }))
                .into();
            }
            if !res.unwrap().is_kyc_done {
                return HttpResponse::BadRequest()
                .json(json!({ "err": "user not completed kyc!!" }))
                .into();
            }
        } 
        // if tx_type is crosschain request then mark src_chain_id,src_nonce is broadcasted by validator
        let mut variant = tx.tx_type.to_string();
        if let Some(s) = variant.find('(') {
            variant = variant[..s].to_string();
        }
        // if crosschain request to other chain
        if variant == "CrossChainRequest".to_string() {
           match &tx.tx_type {
                TxType::CrossChainRequest(hex_str) => {
                    //TODO: sender must be validator
                   let decoded_info =  decode_crosschain_request_type_data(hex_str);
                   if decoded_info.is_err(){
                    return HttpResponse::BadRequest().json(json!({"error":"Error While Decoding Request Type Data"}));
                   }
                   let decoded_info = decoded_info.unwrap();
                   let mut binding = digichain.crosschain.write().unwrap();
                   let res= binding.broadcasted(tx.from, decoded_info.src_chain_id, decoded_info.src_nonce);
                   if res.is_err() {
                     return HttpResponse::AlreadyReported().json(json!({"error":format!("{:?}",res.err())}));
                   }
                }
                _ => {} // it will not reach here
            };
          
        }
        tx.hash = tx.calculate_hash();
        digichain
            .mempool
            .write()
            .unwrap()
            .add_transaction(&tx);
        return HttpResponse::Ok().json(json!({ "data": {
            "tx_hash":tx.hash
        },"id":self.id}));
    }

    pub fn get_chain(
        self,
        digichain: MutexGuard<'_, DigiChain>,
        request_body: Params,
    ) -> HttpResponse {
        let res: Result<GetChainParams, serde_json::Error> =
            serde_json::from_value(request_body.params);
        if res.is_err() {
            return HttpResponse::BadRequest().into();
        }
        let mut params = res.unwrap();
        if params.end_block >= digichain.get_block_number() {
            params.end_block = digichain.get_block_number() - 1u64;
        }
        if params.start_block > digichain.get_block_number() {
            params.start_block = 0u64;
        }
        let chain = digichain.get_chain(
            params.start_block as usize,
            (params.end_block + 1u64) as usize,
        );
        return HttpResponse::Ok().json(json!({ "chain": chain,"id":self.id }));
    }

    pub fn get_mempool(
        self,
        digichain: MutexGuard<'_, DigiChain>,
        request_body: Params,
    ) -> HttpResponse {
        let mempool = digichain.mempool.read().unwrap();
        let mempool = mempool.get_mempool();
        return HttpResponse::Ok().json(json!({ "mempool": {
            "transactions": mempool.transactions,
            "proposals": mempool.get_proposals()
        },"id":self.id }));
    }

    pub fn get_account(
        self,
        digichain: MutexGuard<'_, DigiChain>,
        request_body: Params,
    ) -> HttpResponse {
        let res: Result<GetAccountParams, serde_json::Error> =
            serde_json::from_value(request_body.params);
        if res.is_err() {
            return HttpResponse::BadRequest().into();
        }
        let params = res.unwrap();
        let accounts = digichain.accounts.read().unwrap();
        let res = accounts.get(&params.address);
        if res.is_none() {
            return HttpResponse::Ok().json(json!({"account": {
                "address":params.address,
                "is_registered":false,
                "is_kyc_done":false
            },
                "id":self.id }));
        }
        let res = res.unwrap().read().unwrap();

        return HttpResponse::Ok().json(json!({ "account": {
                "address":res.address,
                "tx_nonce":res.tx_nonce,
                "proposal_nonce":res.proposal_nonce ,
                "is_kyc_done":res.is_kyc_done ,
                "name":res.name ,
                "country":res.country ,
                "mobile":res.mobile ,
                "aadhar_no":res.aadhar_no ,
                "kyc_completed_at":res.kyc_completed_at ,
                "upi_id":res.upi_id ,
                "transactions":res.transactions ,
                "accepts":res.accepts ,
                // "qr_code":res.get_qr_code_data_uri(),
        },"id":self.id }));
    }


    // pub fn get_accounts(
    //     self,
    //     digichain: MutexGuard<'_, DigiChain>,
    //     request_body: Params,
    // ) -> HttpResponse {
    //     let mut data:HashMap<Address,Account> = HashMap::new();
    //     let accounts = digichain.accounts.read().unwrap();
    //     accounts.into_iter().map(|acc| {
    //         data.insert(acc.0,acc.1.read().unwrap().clone());
    //         true
    //     }).collect::<Vec<bool>>();

    //     return HttpResponse::Ok().json(json!({ "accounts": data,"id":self.id }));
    // }


    pub fn get_chain_id(
        self,
        digichain: MutexGuard<'_, DigiChain>,
        request_body: Params,
    ) -> HttpResponse {
        return HttpResponse::Ok().json(json!({ 
            "chain_id": digichain.chain_id
      ,"id":self.id }));
    }

    pub fn get_token(
        self,
        digichain: MutexGuard<'_, DigiChain>,
        request_body: Params,
    ) -> HttpResponse {
        let res: Result<GetTokenParams , serde_json::Error> = serde_json::from_value(request_body.params);
        if res.is_err() {
            return HttpResponse::BadRequest().json(json!(format!("{:?}",res.err()))).into();
        }
        let params: GetTokenParams = res.unwrap();
        let res = self.get_token_by_id(digichain, params.token_id);
        if res.is_err() {
            return HttpResponse::NotFound().into();
        }
        return HttpResponse::Ok().json(json!({ 
            "token": res.unwrap()
      ,"id":self.id }));
    }

    pub fn get_tokens(
        self,
        digichain: MutexGuard<'_, DigiChain>,
        request_body: Params,
    ) -> HttpResponse {
        let res: Result<GetTokensParams , serde_json::Error> = serde_json::from_value(request_body.params);
        if res.is_err() {
            return HttpResponse::BadRequest().json(json!(format!("{:?}",res.err()))).into();
        }
        let  params: GetTokensParams = res.unwrap();
        let no_of_tokens = digichain.get_no_of_tokens();
        let mut from = 0;
        let mut to = no_of_tokens;
        if params.to.is_some() {
            to = params.to.unwrap();
        }
        if params.from.is_some() {
            from = params.from.unwrap();
        }
        if  no_of_tokens > 0u64{
            if to >= no_of_tokens {
                to = no_of_tokens - 1u64;
            }
        }else {
            to = 0u64;
        }
        if from > no_of_tokens {
            from = 0u64;
        }
        let tokens = digichain.get_token(
            from as usize,
            (to + 1u64) as usize,
        );
        return HttpResponse::Ok().json(json!({ 
            "tokens":tokens //NOTE: as of now sending balance of details along with other data
      ,"id":self.id }));
    }

    pub fn get_token_by_chain(
        self,
        digichain: MutexGuard<'_, DigiChain>,
        request_body: Params,
    ) -> HttpResponse {
        let res: Result<GetTokenByChain , serde_json::Error> = serde_json::from_value(request_body.params);
        if res.is_err() {
            return HttpResponse::BadRequest().json(json!(format!("{:?}",res.err()))).into();
        }
        let params: GetTokenByChain = res.unwrap();
        let binding = digichain.chain_id_to_token_mp.read().unwrap();
        let res =  binding.get(&(params.chain_id,params.token_address.to_lowercase()));
        if res.is_none() {
            return HttpResponse::NotFound().into();
        }  
        let binding = digichain.token_list.read().unwrap();
        let res =  binding.get(res.unwrap());
        if res.is_none() {
            return HttpResponse::NotFound().into();
        }  
        return HttpResponse::Ok().json(json!({ 
            "token": res.unwrap()
      ,"id":self.id }));
    }

    pub fn get_contract_config(
        self,
        digichain: MutexGuard<'_, DigiChain>,
        request_body: Params,
    ) -> HttpResponse {
        let res: Result<GetConfigParams , serde_json::Error> = serde_json::from_value(request_body.params);
        if res.is_err() {
            return HttpResponse::BadRequest().json(json!(format!("{:?}",res.err()))).into();
        }
        let params: GetConfigParams = res.unwrap();
        let binding = digichain.crosschain.read().unwrap();
         let configs = binding.get_contracts_config(params.chain_ids);
        return HttpResponse::Ok().json(json!({ 
            "configs": configs
      ,"id":self.id }));
    }
   
    pub fn get_proposals(
        self,
        digichain: MutexGuard<'_, DigiChain>,
        request_body: Params,
    ) -> HttpResponse {
        let res: Result<GetProposalsParams , serde_json::Error> = serde_json::from_value(request_body.params);
        if res.is_err() {
            return HttpResponse::BadRequest().json(json!(format!("{:?}",res.err()))).into();
        }
        let params: GetProposalsParams = res.unwrap();
        let mut ccrequests:Vec<Proposal> = Vec::new();
        let  mempool = digichain.mempool.read().unwrap();
       let proposals= mempool.proposals.read().unwrap();
       let proposals_binding  = proposals.clone().into_iter();
       let _= proposals_binding.map(|hcr| {
        if params.proposal_type.is_some() {
            if hcr.0.to_string() != params.proposal_type.clone().unwrap().to_string() {
                return false;
            } 
        }
       let _ = hcr.1.into_iter().map(|cr| {
            if params.hash.is_some() {
                if params.hash.clone().unwrap() != cr.hash {
                    return false;
                }
            }
            if params.proposed_by.is_some() {
                if params.proposed_by.clone().unwrap() != cr.proposed_by {
                    return false;
                }
                }
                if params.block_number.is_some() {
                    if params.block_number.clone().unwrap() != cr.block_number {
                        return false;
                    }
                    }
    
            ccrequests.push(cr);
            true
        }).collect::<Vec<bool>>();
        true
       }).collect::<Vec<bool>>();

       if ccrequests.len() != 0usize {
        let mut from = 0usize;
        let mut to = ccrequests.len();
        if let Some(_from) = params.from {
         if _from  <= ccrequests.len() as u64{
             from = _from as usize;
         }
        }
        if let Some(_to) = params.to {
         if ccrequests.len() as u64 > 0u64{
                 if _to < ccrequests.len() as u64 {
                     to = ccrequests.len()  - 1usize;
                 }
             }
        }
        ccrequests = ccrequests.get(from..to).unwrap().to_vec();
       }
        return HttpResponse::Ok().json(json!({ 
            "proposals": ccrequests
      ,"id":self.id }));
    }

    pub fn get_validators(
        self,
        digichain: MutexGuard<'_, DigiChain>,
        request_body: Params,
    ) -> HttpResponse {
        let binding = digichain.validators.read().unwrap();
        #[derive(Serialize, Deserialize)]
        struct  ValidatorResponse {
            pub address:Address,
            pub staked: Uint128
        }
        let validators : Vec<ValidatorResponse> =
        binding.clone().into_iter().map( |val| {
            ValidatorResponse {
                address: val.acccount.address,
                staked: val.staked
            }
        }).collect::<Vec<ValidatorResponse>>();
        return HttpResponse::Ok().json(json!({ 
            "validators":  validators
      ,"id":self.id })); 
    }

    pub fn get_transaction(
        self,
        digichain: MutexGuard<'_, DigiChain>,
        request_body: Params,
    ) -> HttpResponse {
        let res: Result<GetTransactionParams , serde_json::Error> = serde_json::from_value(request_body.params);
        if res.is_err() {
            return HttpResponse::BadRequest().json(json!(format!("{:?}",res.err()))).into();
        }
        let params: GetTransactionParams = res.unwrap();

       let binding= digichain.index_transactions.read().unwrap();
       let res = binding.get(&params.tx_hash);
       if res.is_none() {
        return HttpResponse::NotFound().json(json!({ 
            "error":  "tx not found"
      ,"id":self.id })); 
       }
       let bn =res.unwrap();
       let block = digichain.get_block(bn.clone() as u64).unwrap();
       let transaction = block.transactions.into_iter().find(|tx| tx.hash == params.tx_hash).unwrap();
        return HttpResponse::Ok().json(json!({ 
            "transaction":  transaction
      ,"id":self.id })); 
    }

    pub fn get_crosschain_requests(
        self,
        digichain: MutexGuard<'_, DigiChain>,
        request_body: Params,
    ) -> HttpResponse {
        let res: Result<GetCrossChainRequestReadyToExecute , serde_json::Error> = serde_json::from_value(request_body.params);
        if res.is_err() {
            return HttpResponse::BadRequest().json(json!(format!("{:?}",res.err()))).into();
        }
        let params: GetCrossChainRequestReadyToExecute = res.unwrap();
        let binding= digichain.mempool.read().unwrap();
        let mut res = binding.get_crosschain_request_to_execute(params.validator);
        if res.len() != 0usize {
            let mut from = 0usize;
            let mut to = res.len();
            if let Some(_from) = params.from {
             if _from  <= res.len() as u64{
                 from = _from as usize;
             }
            }
            if let Some(_to) = params.to {
             if res.len() as u64 > 0u64{
                     if _to < res.len() as u64 {
                         to = res.len()  - 1usize;
                     }
                 }
            }
            res = res.get(from..to).unwrap().to_vec();
        }
        return HttpResponse::Ok().json(json!({ 
            "crosschain_withdraw_requests":  res
      ,"id":self.id })); 
    } 

    pub fn get_optimal_path(
        self, 
        digichain: MutexGuard<'_, DigiChain>,
        request_body: Params,
    ) -> HttpResponse {
        let res: Result<GetOptimalPath , serde_json::Error> = serde_json::from_value(request_body.params);
        if res.is_err() {
            return HttpResponse::BadRequest().json(json!(format!("{:?}",res.err()))).into();
        }
        let params: GetOptimalPath = res.unwrap();
        let tokens = params.tokens;
        let amounts = params.amounts;
        if amounts.len() != tokens.len() {
            return HttpResponse::BadRequest().json(json!({
                "error":"tokens and amounts length mismatch"
            })).into();
        }
        let mut mp = HashMap::<String,(DigiToken,Uint128)>::new(); // tokenid -> (token_details, max_amount)
        let binding =digichain.token_list.read().unwrap();
        for idx in 0..tokens.len() {
            let res = binding.get(&tokens[idx]);
            if res.is_none() {
                return HttpResponse::NotFound().json(json!({
                    "error":"token not found"
                })).into();
    
            }
            mp.insert(tokens[idx].clone(), (res.unwrap().clone(),amounts[idx]));
        }
        let mut rng = rand::thread_rng();
        let mut indices: Vec<usize> = (0..tokens.len()).collect();
        indices.shuffle(&mut rng);
        let mut use_tokens:Vec<String> = Vec::new();
        let mut use_amounts:Vec<Uint128> = Vec::new();
        let mut max_amount:Uint128 = Uint128::from(0u128); // max can be reached
        for idx in 0..tokens.len() {
            let idx = indices[idx];
            let (dt,amount) = mp.get(&tokens[idx]).unwrap().clone();
            if amount > Uint128::from(0u128) {
                let mut left = Uint128::from(0u128);
                let mut right = amount; // Adjust the range based on your requirements
                let mut closest_value = Uint128::from(std::u128::MAX); // Initialize to a large value
                let mut lmax_amount = Uint128::from(0u128);
                while left <= right {
                    let mid = left + (right - left) / Uint128::from(2u128);
                    let tvalue = (dt.price * mid) / Uint128::from((10u128).pow(dt.decimal as u32));
                    let current_value = max_amount + tvalue;

                    if abs(params.amount,current_value) <= abs(params.amount,closest_value){
                        closest_value = current_value;
                        lmax_amount = mid;
                    }
                    if current_value < params.amount {
                        // Move the left boundary to the right
                        left = mid + Uint128::from(1u128);
                    } else {
                        // Move the right boundary to the left
                        if mid == Uint128::from(0u128) {break}; // TODO: check
                        right = mid - Uint128::from(1u128);
                    }
                }
                max_amount = closest_value;
                use_amounts.push(lmax_amount);
                use_tokens.push(tokens[idx].clone());
            }
        }
        return HttpResponse::Ok().json(json!({ 
            "data": {
                "use_tokens":use_tokens,
                "use_amounts":use_amounts, // divide by 10^18
                "max_amount":max_amount
            }
      ,"id":self.id }));
    }

    pub fn is_broadcasted(
        self,
        digichain: MutexGuard<'_, DigiChain>,
        request_body: Params,
    ) -> HttpResponse {
        let res: Result<IsBroadcastedParams , serde_json::Error> = serde_json::from_value(request_body.params);
        if res.is_err() {
            return HttpResponse::BadRequest().json(json!(format!("{:?}",res.err()))).into();
        } 
        let params: IsBroadcastedParams = res.unwrap();
        let binding  = digichain.crosschain.read().unwrap();
      let res=  binding.is_broadcasted(params.validator, params.src_chain_id, params.src_nonce);
        return HttpResponse::Ok().json(json!({ 
            "is_broadcasted": res
      ,"id":self.id }));
    }


    pub fn airdrop(
        self,
        digichain: MutexGuard<'_, DigiChain>,
        request_body: Params,
    ) -> HttpResponse {
        let res: Result<AirDropParams , serde_json::Error> = serde_json::from_value(request_body.params);
        if res.is_err() {
            return HttpResponse::BadRequest().json(json!(format!("{:?}",res.err()))).into();
        }
        let params: AirDropParams = res.unwrap();
        let mut binding= digichain.token_list.write().unwrap();
       let  token: Option<&mut DigiToken>= binding.get_mut(&params.token);
       if token.is_none() {
            return HttpResponse::NotFound().json(json!({ 
                "error":  "token not found"
        ,"id":self.id })); 
       }
       let  token = token.unwrap();
        return HttpResponse::Ok().json(json!({ 
            "minted":  token.mint(params.address, params.amount)
      ,"id":self.id })); 
    }

    pub fn pause(
        self,
        digichain: MutexGuard<'_, DigiChain>,
        request_body: Params,
    ) -> HttpResponse {
        let res: Result<PauseAndUnPauseParams , serde_json::Error> = serde_json::from_value(request_body.params);
        if res.is_err() {
            return HttpResponse::BadRequest().json(json!(format!("{:?}",res.err()))).into();
        }
        let params: PauseAndUnPauseParams = res.unwrap();
        *digichain.pause.write().unwrap() = params.pause;
        return HttpResponse::Ok().json(json!({ 
            "paused":  params.pause
      ,"id":self.id })); 
    }


    pub fn get_transactions(
        self,
        digichain: MutexGuard<'_, DigiChain>,
        request_body: Params,
    ) -> HttpResponse {
        let res: Result<GetTransactionsParams , serde_json::Error> = serde_json::from_value(request_body.params);
        if res.is_err() { 
            return HttpResponse::BadRequest().json(json!(format!("{:?}",res.err()))).into();
        }
        let params: GetTransactionsParams = res.unwrap();

        if params.address.is_some(){
            let address = params.address.unwrap();
            let binding= digichain.accounts.read().unwrap();
            let acc = binding.get(&address);
            if acc.is_none() {
                return HttpResponse::NotFound().json(json!({ 
                    "error":  "user not found"
              ,"id":self.id })); 
            }
            let acc = acc.unwrap();
            let binding = acc.read().unwrap().transactions.clone();
            let txs:Vec<Transaction>=  binding.into_iter().filter_map(|t| {
                let binding= digichain.index_transactions.read().unwrap();
                let res = binding.get(&t);
                if res.is_none() {
                return  None;
                }
                let bn =res.unwrap();
                let block = digichain.get_block(bn.clone() as u64).unwrap();
                let transaction = block.transactions.into_iter().find(|tx| tx.hash == t).unwrap();
                Some(transaction)    
            }).collect::<Vec<Transaction>>();

            return HttpResponse::Ok().json(json!({ 
                "transactions":  txs
          ,"id":self.id })); 
        }

        return HttpResponse::NotFound().json(json!({ 
            "error":  "options not supported"
      ,"id":self.id })); 
    }

}

#[derive(Clone, Debug, JsonSchema, Serialize, Deserialize)]
pub struct Params {
    id: String,
    method: String,
    params: Value,
}

pub async fn handler(
    req: HttpRequest,
    digichain: web::Data<Arc<Mutex<DigiChain>>>,
    body: web::Json<Params>,
) -> impl Responder {
    let request_body: Params = body.0;
    let res = digichain.lock();
    if res.is_err() {
        return HttpResponse::BadRequest().into();
    }
    let digichain = res.unwrap();
    let res = digichain.json_rpc.read();
    if res.is_err() {
        return HttpResponse::BadRequest().into();
    }
    let json_rpc = res.unwrap().clone();

    // Match based on the value of the "method" field
    match request_body.method.as_str() {
        "get_block_number" => JsonRpc::get_block_number(json_rpc, digichain, request_body),
        "get_chain" => JsonRpc::get_chain(json_rpc, digichain, request_body),
        "broadcast_transaction" => {
            JsonRpc::broadcast_transaction(json_rpc, digichain, request_body)
        }
        "is_broadcasted" => {
            JsonRpc::is_broadcasted(json_rpc, digichain, request_body)
        }
        "get_mempool" => JsonRpc::get_mempool(json_rpc, digichain, request_body),
        "get_account" => JsonRpc::get_account(json_rpc, digichain, request_body),
        "get_chain_id" => JsonRpc::get_chain_id(json_rpc, digichain, request_body),
        "get_token" => JsonRpc::get_token(json_rpc, digichain, request_body),
        "get_tokens" => JsonRpc::get_tokens(json_rpc, digichain, request_body),
        "get_token_by_chain" => JsonRpc::get_token_by_chain(json_rpc, digichain, request_body),
        "get_contracts_config" => JsonRpc::get_contract_config(json_rpc, digichain, request_body),
        "get_validators" => JsonRpc::get_validators(json_rpc, digichain, request_body),
        "get_proposals" => JsonRpc::get_proposals(json_rpc, digichain, request_body),
        "balance_of" => JsonRpc::get_balance_of(json_rpc, digichain, request_body),
        "get_balances" => JsonRpc::get_balances(json_rpc, digichain, request_body),
        "get_transaction" => JsonRpc::get_transaction(json_rpc, digichain, request_body),
        "get_crosschain_requests" => JsonRpc::get_crosschain_requests(json_rpc, digichain, request_body),
        "get_optimal_path" => JsonRpc::get_optimal_path(json_rpc, digichain, request_body),

        "get_transactions" => JsonRpc::get_transactions(json_rpc, digichain, request_body),


        //util fn for testing
        "airdrop" => JsonRpc::airdrop(json_rpc, digichain, request_body),
        "pause" => JsonRpc::pause(json_rpc, digichain, request_body),
        _ => {
            return HttpResponse::Ok().body("sss".to_string());
        }
    }
}



fn to_18_decimal(amount:u128,id:u8) -> u128 {
    let t18 = (10u128).pow(18 as u32);
    let tin = (10u128).pow(id as u32);
    return amount * t18 / tin;
}

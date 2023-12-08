use actix_web::{middleware, web, App, HttpServer};
use cosmwasm_std::Uint128;
use digichain::acccount::Account;
use digichain::crosschain::CrossChain;
use digichain::json_rpc::{self, JsonRpc};
use digichain::mempool::Mempool;
use digichain::types::Address;
use digichain::validators::Validator;
use digichain::{block::DigiBlock, digichain::DigiChain};
use dotenv::dotenv;
use ethers_signers::Wallet;
use std::collections::HashMap;
use std::env;
use std::str::FromStr;
use std::sync::{Arc, Mutex, RwLock};
use std::time::SystemTime;

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    dotenv().ok();
    let res = env::var("ADDRESS");
    if res.is_err() {
        panic!("define ADDRESS in .env file");
    }
    let private_key_res = env::var("PRIVATE_KEY");
    if private_key_res.is_err() {
        panic!("define PRIVATE_KEY in .env file");
    }

    println!("Starting Chain!!");
    let account = Account::new(Address::from_str(&res.unwrap()).unwrap());
    let wallet = Wallet::from_str(&private_key_res.unwrap()).unwrap();
    let validator = Validator::new(account.clone(), Uint128::from(100u128), wallet);
    let timestamp = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let genesis_block = DigiBlock::create_block(
        validator.clone(),
        timestamp,
        0u64,
        "".to_string(),
        vec![],
        vec![],
    );
    let mut accounts = HashMap::new();
    accounts.insert(account.address.clone(), Arc::new(RwLock::new(account)));
    let crosschain = CrossChain::new("11".to_string(), HashMap::new());
    let digichain = DigiChain {
        pause: Arc::new(RwLock::new(false)),
        chain_id: String::from("11"),
        mempool: Arc::new(RwLock::new(Mempool::new())),
        validator: Arc::new(RwLock::new(validator.clone())),
        json_rpc: Arc::new(RwLock::new(JsonRpc::new())),
        blocks: Arc::new(RwLock::new(vec![Arc::new(RwLock::new(genesis_block))])),
        token_list: Arc::new(RwLock::new(HashMap::new())),
        accounts: Arc::new(RwLock::new(accounts)),
        chain_id_to_token_mp: Arc::new(RwLock::new(HashMap::new())),
        crosschain: Arc::new(RwLock::new(crosschain)),
        validators: Arc::new(RwLock::new(vec![validator])),
        index_transactions: Arc::new(RwLock::new(HashMap::new())),
        index_proposals: Arc::new(RwLock::new(HashMap::new())),
    };

    // init json rpc
    env::set_var("RUST_LOG", "actix_web=debug,actix_server=info");
    env_logger::init();

    let digichain_arc = Arc::new(Mutex::new(digichain.clone()));
    tokio::spawn(async move {
        DigiChain::add_blocks(&mut digichain.clone()).await;
    });

    HttpServer::new(move || {
        App::new()
            .wrap(middleware::Logger::default())
            .app_data(web::Data::new(digichain_arc.clone()))
            .route("/", web::post().to(json_rpc::handler))
        // .service(json_rpc::handle)
    })
    .bind("127.0.0.1:8000")?
    .run()
    .await
}

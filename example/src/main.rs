/// There are 10^6 micro-lamports in one lamport
const MICRO_LAMPORTS_PER_LAMPORT: u64 = 1_000_000;
const BPS: u16 = 100;

use std::env;

use jupiter_swap_api_client::transaction_config::ComputeUnitPriceMicroLamports;
use jupiter_swap_api_client::{
    quote::QuoteRequest, swap::SwapRequest, transaction_config::TransactionConfig,
    JupiterSwapApiClient,
};

use solana_client::nonblocking::rpc_client::RpcClient;
use solana_sdk::signer::Signer;
use solana_sdk::{hash::Hash, pubkey::Pubkey};
use solana_sdk::{pubkey, transaction::VersionedTransaction};
use tokio;

const USDC_MINT: Pubkey = pubkey!("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v");
const NATIVE_MINT: Pubkey = pubkey!("So11111111111111111111111111111111111111112");
const WENT_MINT: Pubkey = pubkey!("WENWENvqqNya429ubCdR81ZmD69brwQaaBYY6p3LCpk");

const KEYPAIR_PATH: &str = ""; // change to your keypair path
const RPC_URL: &str = "https://api.mainnet-beta.solana.com"; // change to your RPC URL
const PRIORITY_RATE: f64 = 0.001; // set the compute unit price in lamports
const SLIPPAGE: u16 = 20; // slippage in percentage

#[tokio::main]
async fn main() {
    let file = std::fs::File::open(KEYPAIR_PATH).unwrap();
    let data: serde_json::Value = serde_json::from_reader(file).unwrap();
    let key_bytes: Vec<u8> = serde_json::from_value(data.clone()).unwrap();
    let trading_keypair = solana_sdk::signature::Keypair::from_bytes(&key_bytes).unwrap();

    let api_base_url = env::var("API_BASE_URL").unwrap_or("https://quote-api.jup.ag/v6".into());

    let jupiter_swap_api_client = JupiterSwapApiClient::new(api_base_url);

    let quote_request: QuoteRequest = QuoteRequest {
        amount: 1_000_000,
        input_mint: NATIVE_MINT,
        output_mint: WENT_MINT,
        slippage_bps: SLIPPAGE*BPS,
        ..QuoteRequest::default()
    };

    // GET /quote
    let quote_response = jupiter_swap_api_client.quote(&quote_request).await.unwrap();
    println!("{quote_response:#?}");

    // Create SwapRequest with the provided values, including modifying the compute_unit_price_micro_lamports
    let mut swap_request = SwapRequest {
        user_public_key: trading_keypair.pubkey(),
        quote_response: quote_response.clone(),
        config: TransactionConfig::default(),
    };

    // Modify the compute_unit_price_micro_lamports value
    let new_compute_unit_price = ComputeUnitPriceMicroLamports::MicroLamports(
        (PRIORITY_RATE * MICRO_LAMPORTS_PER_LAMPORT as f64) as u64,
    );
    swap_request
        .config
        .set_compute_unit_price_micro_lamports(new_compute_unit_price);

    // POST /swap
    let swap_response = jupiter_swap_api_client.swap(&swap_request).await.unwrap();

    println!("Raw tx len: {}", swap_response.swap_transaction.len());

    let mut versioned_transaction: VersionedTransaction =
        bincode::deserialize(&swap_response.swap_transaction).unwrap();

    // Send with rpc client...
    let rpc_client = RpcClient::new(RPC_URL.into());

    // Fetch the latest blockhash
    let recent_blockhash: Hash = match rpc_client.get_latest_blockhash().await {
        Ok(blockhash) => blockhash,
        Err(err) => {
            eprintln!("Error fetching latest blockhash: {:?}", err);
            return;
        }
    };

    versioned_transaction
        .message
        .set_recent_blockhash(recent_blockhash);

    let signed_versioned_transaction =
        VersionedTransaction::try_new(versioned_transaction.message, &[&trading_keypair]).unwrap();

    // Broadcast transaction to solana blockchain
    let error = rpc_client
        .send_and_confirm_transaction(&signed_versioned_transaction)
        .await
        .unwrap_err();
    println!("{error}");

    // POST /swap-instructions
    let swap_instructions = jupiter_swap_api_client
        .swap_instructions(&SwapRequest {
            user_public_key: trading_keypair.pubkey(),
            quote_response,
            config: TransactionConfig::default(),
        })
        .await
        .unwrap();
    println!("swap_instructions: {swap_instructions:?}");
}

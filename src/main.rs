mod compute;
mod taiko;

use std::env;

use alloy::sol_types::SolEventInterface;
use alloy::{
    primitives::{Address, U256},
    providers::{Provider, ProviderBuilder},
};
use clap::Parser;
use compute::{L2Client, compute_batch_revenue, format_wei_eth};

#[derive(Parser, Debug)]
#[command(name = "taiko-batch-revenue")]
#[command(about = "Compute Taiko L2 batch revenue from L1 BatchProposed events", long_about = None)]
struct Cli {
    /// L1 block number to scan for BatchProposed
    #[arg(long, value_parser = clap::value_parser!(u64))]
    l1_block: u64,
}

#[tokio::main]
async fn main() -> eyre::Result<()> {
    color_eyre::install().ok();

    let cli = Cli::parse();

    let l1_url = env::var("L1_RPC_URL").expect("L1_RPC_URL not set");
    let l2_client = L2Client::from_env()
        .await
        .expect("L2_RPC_URL not set or invalid");
    let inbox_addr: Address = env::var("TAIKO_INBOX_ADDRESS")
        .expect("TAIKO_INBOX_ADDRESS not set")
        .parse()
        .expect("Invalid TAIKO_INBOX_ADDRESS");

    let l1 = ProviderBuilder::new().connect_http(l1_url.parse()?);
    // Validate both RPC endpoints are reachable
    let _ = l1
        .get_chain_id()
        .await
        .expect("L1 RPC not reachable or invalid");
    l2_client
        .ping()
        .await
        .expect("L2 RPC not reachable or invalid");

    // Build the contract interface using the generated instance
    let inbox = taiko::ITaikoInbox::new(inbox_addr, l1.clone());

    // Filter BatchProposed in the specified L1 block
    let filter = inbox
        .BatchProposed_filter()
        .from_block(cli.l1_block)
        .to_block(cli.l1_block)
        .filter;

    let logs = l1.get_logs(&filter).await?;

    if logs.is_empty() {
        println!("No BatchProposed events found in L1 block {}", cli.l1_block);
        return Ok(());
    }

    for (idx, log) in logs.into_iter().enumerate() {
        let raw = log.inner;
        let decoded =
            taiko::ITaikoInbox::ITaikoInboxEvents::decode_log(&raw).expect("decode event");
        if let taiko::ITaikoInbox::ITaikoInboxEvents::BatchProposed(batch) = decoded.data {
            let blocks = batch.block_numbers_proposed();
            if blocks.is_empty() {
                println!("Event #{idx}: empty batch (no blocks)");
                continue;
            }

            let first = *blocks.first().unwrap();
            let last = *blocks.last().unwrap();
            println!(
                "Event #{idx}: batch_id={} blocks {}..={} ({} blocks)",
                batch.meta.batchId,
                first,
                last,
                blocks.len()
            );

            let per_block = compute_batch_revenue(&l2_client, &blocks)
                .await
                .expect("compute revenue");

            let mut total_paid = U256::from(0);
            let mut total_txs = 0usize;
            for b in &per_block {
                total_paid = total_paid.saturating_add(b.total_paid_wei);
                total_txs += b.tx_count;
            }

            println!("  total txs: {}", total_txs);
            println!("  total paid: {}", format_wei_eth(&total_paid));
        }
    }

    Ok(())
}

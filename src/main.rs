mod compute;
mod taiko;

use std::env;

use alloy::sol_types::SolEventInterface;
use alloy::{
    primitives::{Address, U256},
    providers::{Provider, ProviderBuilder},
};
use clap::Parser;
use compute::{L2Client, compute_batch_revenue, compute_block_revenue, format_wei_eth};

#[derive(Parser, Debug)]
#[command(name = "taiko-batch-revenue")]
#[command(about = "Compute Taiko L2 revenue: either single L2 block or from L1 BatchProposed events", long_about = None)]
struct Cli {
    /// L2 block number to compute revenue for (mutually exclusive with --l1-block)
    #[arg(long, value_parser = clap::value_parser!(u64), conflicts_with = "l1_block")]
    l2_block: Option<u64>,

    /// L1 block number to scan for BatchProposed (required unless --l2-block is provided)
    #[arg(long, value_parser = clap::value_parser!(u64), required_unless_present = "l2_block", conflicts_with = "l2_block")]
    l1_block: Option<u64>,

    /// Optional: scan a range of L1 blocks (e.g., --range 100 to scan 100 blocks before and after)
    #[arg(long, value_parser = clap::value_parser!(u64), requires = "l1_block")]
    range: Option<u64>,
}

#[tokio::main]
async fn main() -> eyre::Result<()> {
    color_eyre::install().ok();

    let cli = Cli::parse();

    let l2_client = L2Client::from_env()
        .await
        .expect("L2_RPC_URL not set or invalid");
    // Always validate L2 RPC is reachable
    l2_client
        .ping()
        .await
        .expect("L2 RPC not reachable or invalid");

    // If user requested a single L2 block, compute and exit early
    if let Some(l2_block) = cli.l2_block {
        println!("Computing revenue for L2 block {l2_block}...");
        let res = compute_block_revenue(&l2_client, l2_block)
            .await
            .expect("compute revenue");
        println!("  txs: {}", res.tx_count);
        println!("  total paid: {}", format_wei_eth(&res.total_paid_wei));
        return Ok(());
    }

    // Otherwise, we are scanning L1 for BatchProposed events
    let l1_block = cli.l1_block.expect("--l1-block is required unless --l2-block is provided");
    let l1_url = env::var("L1_RPC_URL").expect("L1_RPC_URL not set");
    let inbox_addr: Address = env::var("TAIKO_INBOX_ADDRESS")
        .expect("TAIKO_INBOX_ADDRESS not set")
        .parse()
        .expect("Invalid TAIKO_INBOX_ADDRESS");
    let l1 = ProviderBuilder::new().connect_http(l1_url.parse()?);
    // Validate L1 RPC endpoint is reachable
    let _ = l1
        .get_chain_id()
        .await
        .expect("L1 RPC not reachable or invalid");
    // Build the contract interface using the generated instance
    let inbox = taiko::ITaikoInbox::new(inbox_addr, l1.clone());

    // Determine block range
    let (from_block, to_block) = if let Some(range) = cli.range {
        let from = l1_block.saturating_sub(range);
        let to = l1_block.saturating_add(range);
        (from, to)
    } else {
        (l1_block, l1_block)
    };

    println!("Scanning for BatchProposed events:");
    println!("  Contract: {}", inbox_addr);
    println!("  L1 blocks: {} to {}", from_block, to_block);

    // Filter BatchProposed in the specified L1 block range
    let filter = inbox
        .BatchProposed_filter()
        .from_block(from_block)
        .to_block(to_block)
        .filter;

    let logs = l1.get_logs(&filter).await?;

    // Also try to get ALL logs from this contract in this block range for debugging
    let all_logs_filter = alloy::rpc::types::Filter::new()
        .address(inbox_addr)
        .from_block(from_block)
        .to_block(to_block);

    let all_logs = l1.get_logs(&all_logs_filter).await?;
    println!(
        "  Total logs from contract in block range: {}",
        all_logs.len()
    );

    if !all_logs.is_empty() && logs.is_empty() {
        println!(
            "  Found {} logs but no BatchProposed events. Event signatures found:",
            all_logs.len()
        );
        for log in all_logs.iter().take(5) {
            if let Some(topic0) = log.topics().first() {
                println!("    - {}", topic0);
            }
        }
    }

    if logs.is_empty() {
        println!(
            "No BatchProposed events found in L1 blocks {} to {}",
            from_block, to_block
        );
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

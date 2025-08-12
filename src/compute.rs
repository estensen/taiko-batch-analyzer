use alloy::transports::http::reqwest::Url;
use alloy::{
    network::TransactionResponse,
    primitives::{B256, U256},
    providers::{Provider, ProviderBuilder},
    rpc::types::TransactionReceipt,
};
use std::env;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ComputeError {
    #[error("missing env var {0}")]
    MissingEnv(&'static str),
    #[error("rpc error: {0}")]
    Rpc(String),
}

pub struct L2Client {
    pub url: Url,
}

impl L2Client {
    pub async fn from_env() -> Result<Self, ComputeError> {
        let url = env::var("L2_RPC_URL").map_err(|_| ComputeError::MissingEnv("L2_RPC_URL"))?;
        let parsed: Url = url
            .parse::<Url>()
            .map_err(|e| ComputeError::Rpc(e.to_string()))?;
        Ok(Self { url: parsed })
    }

    pub async fn block_tx_hashes(
        &self,
        number: u64,
    ) -> Result<Vec<alloy::primitives::B256>, ComputeError> {
        let provider = ProviderBuilder::new().connect_http(self.url.clone());
        let block = provider
            .get_block_by_number(number.into())
            .await
            .map_err(|e| ComputeError::Rpc(e.to_string()))?;
        let Some(block) = block else {
            return Ok(vec![]);
        };
        Ok(match block.transactions {
            alloy::rpc::types::BlockTransactions::Full(txs) => {
                txs.into_iter().map(|t| t.tx_hash()).collect()
            }
            alloy::rpc::types::BlockTransactions::Hashes(hashes) => hashes.into_iter().collect(),
            alloy::rpc::types::BlockTransactions::Uncle => vec![],
        })
    }

    pub async fn tx_receipt(&self, hash: B256) -> Result<Option<TransactionReceipt>, ComputeError> {
        let provider = ProviderBuilder::new().connect_http(self.url.clone());
        provider
            .get_transaction_receipt(hash)
            .await
            .map_err(|e| ComputeError::Rpc(e.to_string()))
    }

    pub async fn ping(&self) -> Result<(), ComputeError> {
        let provider = ProviderBuilder::new().connect_http(self.url.clone());
        // A lightweight call that requires a working RPC
        let _ = provider
            .get_chain_id()
            .await
            .map_err(|e| ComputeError::Rpc(e.to_string()))?;
        Ok(())
    }
}

pub struct RevenueResult {
    pub block_number: u64,
    pub tx_count: usize,
    pub total_paid_wei: U256,
}

pub async fn compute_block_revenue(
    client: &L2Client,
    block_number: u64,
) -> Result<RevenueResult, ComputeError> {
    let tx_hashes = client.block_tx_hashes(block_number).await?;

    // Fetch receipts concurrently with a simple bounded approach
    let mut total_paid = U256::from(0);
    let mut tx_count = 0usize;

    // Small batches to avoid rate limits
    const CHUNK: usize = 64;
    for chunk in tx_hashes.chunks(CHUNK) {
        let futs = chunk.iter().cloned().map(|h| client.tx_receipt(h));
        let results = futures::future::join_all(futs).await;
        for r in results {
            if let Ok(Some(rcpt)) = r {
                tx_count += 1;
                let gas_used = U256::from(rcpt.gas_used);
                let effective_gas_price = U256::from(rcpt.effective_gas_price);
                total_paid = total_paid.saturating_add(effective_gas_price * gas_used);
            }
        }
    }

    Ok(RevenueResult {
        block_number,
        tx_count,
        total_paid_wei: total_paid,
    })
}

pub async fn compute_batch_revenue(
    client: &L2Client,
    blocks: &[u64],
) -> Result<Vec<RevenueResult>, ComputeError> {
    // Compute per block concurrently, but limit inflight
    const MAX_INFLIGHT: usize = 16;
    let mut results: Vec<RevenueResult> = Vec::with_capacity(blocks.len());

    let mut idx = 0;
    while idx < blocks.len() {
        let end = (idx + MAX_INFLIGHT).min(blocks.len());
        let futs = blocks[idx..end]
            .iter()
            .copied()
            .map(|b| compute_block_revenue(client, b));
        let chunk_results = futures::future::join_all(futs).await;
        for r in chunk_results {
            if let Ok(res) = r {
                results.push(res);
            }
        }
        idx = end;
    }

    // Sort by block number just in case
    results.sort_by_key(|r| r.block_number);
    Ok(results)
}

pub fn format_wei_eth(value: &U256) -> String {
    // Convert to ETH string with 18 decimals
    let wei_str = value.to_string();
    // Simple formatting using parse_units for denominating when needed
    // We'll output both raw wei and approx ETH
    let eth =
        alloy::primitives::utils::format_units(*value, "ether").unwrap_or_else(|_| "0".into());
    format!("{wei} wei (~{eth} ETH)", wei = wei_str, eth = eth)
}

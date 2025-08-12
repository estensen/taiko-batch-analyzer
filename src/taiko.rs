#![allow(clippy::uninlined_format_args)]
#![allow(clippy::cognitive_complexity)]

use alloy::sol;

sol! {
    #[allow(missing_docs)]
    #[sol(rpc)]
    #[derive(Debug, serde::Serialize, serde::Deserialize)]
    interface ITaikoInbox {
        #[derive(Default)]
        event BatchProposed(BatchInfo info, BatchMetadata meta, bytes txList);
        #[derive(Default)]
        event BatchesProved(address verifier, uint64[] batchIds, Transition[] transitions);
        #[derive(Default)]
        event BatchesVerified(uint64 batchId, bytes32 blockHash);

        #[derive(Default)]
        struct BaseFeeConfig {
            uint8 adjustmentQuotient;
            uint8 sharingPctg;
            uint32 gasIssuancePerSecond;
            uint64 minGasExcess;
            uint32 maxGasIssuancePerBlock;
        }

        #[derive(Default)]
        struct BlockParams {
            uint16 numTransactions;
            uint8 timeShift;
            bytes32[] signalSlots;
        }

        #[derive(Default)]
        struct BatchInfo {
            bytes32 txsHash;
            BlockParams[] blocks;
            bytes32[] blobHashes;
            bytes32 extraData;
            address coinbase;
            uint64 proposedIn;
            uint64 blobCreatedIn;
            uint32 blobByteOffset;
            uint32 blobByteSize;
            uint32 gasLimit;
            uint64 lastBlockId;
            uint64 lastBlockTimestamp;
            uint64 anchorBlockId;
            bytes32 anchorBlockHash;
            BaseFeeConfig baseFeeConfig;
        }

        #[derive(Default)]
        struct BatchMetadata {
            bytes32 infoHash;
            address proposer;
            uint64 batchId;
            uint64 proposedAt;
        }

        #[derive(Default)]
        struct Transition {
            bytes32 parentHash;
            bytes32 blockHash;
            bytes32 stateRoot;
        }

        function getBatch(uint64 batchId) public view returns (Batch memory);

        struct Batch {
            bytes32 metaHash;
            uint64 lastBlockId;
            uint96 reserved3;
            uint96 livenessBond;
            uint64 batchId;
            uint64 lastBlockTimestamp;
            uint64 anchorBlockId;
            uint24 nextTransitionId;
            uint8 reserved4;
            uint24 verifiedTransitionId;
        }
    }
}

impl ITaikoInbox::BatchProposed {
    /// Returns the block numbers that were proposed in this batch, by looking
    /// at the `info.blocks` and `lastBlockId` fields.
    pub fn block_numbers_proposed(&self) -> Vec<u64> {
        let last = self.info.lastBlockId;
        let count = self.info.blocks.len() as u64;

        if last == 0 && count > 0 {
            return vec![0];
        }

        let first = last.saturating_sub(count) + 1;
        (first..=last).collect()
    }

    pub const fn last_block_number(&self) -> u64 {
        self.info.lastBlockId
    }
    pub const fn last_block_timestamp(&self) -> u64 {
        self.info.lastBlockTimestamp
    }
}

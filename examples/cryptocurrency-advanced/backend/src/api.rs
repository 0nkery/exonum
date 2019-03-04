// Copyright 2019 The Exonum Team
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//   http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! Cryptocurrency API.

use exonum::{
    api::{self, ServiceApiBuilder, ServiceApiState},
    blockchain::{self, BlockProof, TransactionMessage},
    crypto::{Hash, PublicKey},
    explorer::{BlockchainExplorer, TransactionInfo},
    helpers::Height,
    storage::{ListProof, MapProof},
};

use crate::{wallet::Wallet, Schema, CRYPTOCURRENCY_SERVICE_ID};

/// Describes the query parameters for the `get_wallet` endpoint.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct WalletQuery {
    /// Public key of the queried wallet.
    pub pub_key: PublicKey,
}

/// Proof of existence for specific wallet.
#[derive(Debug, Serialize, Deserialize)]
pub struct WalletProof {
    /// Proof of the whole database table.
    pub to_table: MapProof<Hash, Hash>,
    /// Proof of the specific wallet in this table.
    pub to_wallet: MapProof<PublicKey, Wallet>,
}

/// Wallet history.
#[derive(Debug, Serialize, Deserialize)]
pub struct WalletHistory {
    /// Proof of the list of transaction hashes.
    pub proof: ListProof<Hash>,
    /// List of above transactions.
    pub transactions: Vec<TransactionMessage>,
}

/// Wallet information.
#[derive(Debug, Serialize, Deserialize)]
pub struct WalletInfo {
    /// Proof of the last block.
    pub block_proof: BlockProof,
    /// Proof of the appropriate wallet.
    pub wallet_proof: WalletProof,
    /// History of the appropriate wallet.
    pub wallet_history: Option<WalletHistory>,
}

/// Transaction hash and block height at which it's been committed.
#[derive(Debug, Serialize, Deserialize)]
pub struct SimpleTransactionInfo {
    /// Transaction's hash.
    hash: Hash,
    /// Transaction's block height.
    height: Height,
}

/// Simplified wallet information.
#[derive(Debug, Serialize, Deserialize)]
pub struct SimpleWalletInfo {
    /// List of transactions for a given wallet.
    pub transactions: Vec<SimpleTransactionInfo>,
}

/// Public service API description.
#[derive(Debug, Clone, Copy)]
pub struct PublicApi;

impl PublicApi {
    /// Endpoint for getting a single wallet.
    pub fn wallet_info(state: &ServiceApiState, query: WalletQuery) -> api::Result<WalletInfo> {
        let snapshot = state.snapshot();
        let general_schema = blockchain::Schema::new(&snapshot);
        let currency_schema = Schema::new(&snapshot);

        let max_height = general_schema.block_hashes_by_height().len() - 1;

        let block_proof = general_schema
            .block_and_precommits(Height(max_height))
            .unwrap();

        let to_table: MapProof<Hash, Hash> =
            general_schema.get_proof_to_service_table(CRYPTOCURRENCY_SERVICE_ID, 0);

        let to_wallet: MapProof<PublicKey, Wallet> =
            currency_schema.wallets().get_proof(query.pub_key);

        let wallet_proof = WalletProof {
            to_table,
            to_wallet,
        };

        let wallet = currency_schema.wallet(&query.pub_key);

        let explorer = BlockchainExplorer::new(state.blockchain());

        let wallet_history = wallet.map(|_| {
            let history = currency_schema.wallet_history(&query.pub_key);
            let proof = history.get_range_proof(0, history.len());

            let transactions = history
                .iter()
                .map(|record| explorer.transaction_without_proof(&record).unwrap())
                .collect::<Vec<_>>();

            WalletHistory {
                proof,
                transactions,
            }
        });

        Ok(WalletInfo {
            block_proof,
            wallet_proof,
            wallet_history,
        })
    }

    /// Endpoint for getting a list of transaction hashes and block height at
    /// which they've been committed for a single wallet identified by public
    /// key.
    pub fn simple_wallet_info(
        state: &ServiceApiState,
        query: WalletQuery,
    ) -> api::Result<SimpleWalletInfo> {
        let snapshot = state.snapshot();
        let currency_schema = Schema::new(&snapshot);

        // Check if wallet exists.
        let _wallet = currency_schema.wallet(&query.pub_key).ok_or_else(|| {
            api::error::Error::NotFound(format!(
                "Wallet with public key = {} is not found",
                query.pub_key
            ))
        })?;

        let explorer = BlockchainExplorer::new(state.blockchain());

        let history = currency_schema.wallet_history(&query.pub_key);
        let transactions = history
            .iter()
            .filter_map(|hash| match explorer.transaction(&hash) {
                Some(TransactionInfo::Committed(transaction)) => Some(SimpleTransactionInfo {
                    height: transaction.location().block_height(),
                    hash,
                }),
                _ => None,
            })
            .collect::<Vec<_>>();

        Ok(SimpleWalletInfo { transactions })
    }

    /// Wires the above endpoint to public scope of the given `ServiceApiBuilder`.
    pub fn wire(builder: &mut ServiceApiBuilder) {
        builder
            .public_scope()
            .endpoint("v1/wallets/info", Self::wallet_info)
            .endpoint("v1/wallets/info/simple", Self::simple_wallet_info);
    }
}

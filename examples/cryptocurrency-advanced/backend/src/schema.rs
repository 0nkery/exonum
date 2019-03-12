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

//! Cryptocurrency database schema.

use exonum::{
    crypto::{Hash, PublicKey},
    storage::{Fork, ProofListIndex, ProofMapIndex, Snapshot},
};

use crate::{multisig_transfer::MultisignatureTransfer, wallet::Wallet, INITIAL_BALANCE};

const WALLET_TABLE: &str = "cryptocurrency.wallets";
const WALLET_HISTORY_FAMILY: &str = "cryptocurrency.wallet_history";
const MULTISIG_TRANSFER_TABLE: &str = "cryptocurrency.multisig_transfers";

/// Database schema for the cryptocurrency.
#[derive(Debug)]
pub struct Schema<T> {
    view: T,
}

impl<T> AsMut<T> for Schema<T> {
    fn as_mut(&mut self) -> &mut T {
        &mut self.view
    }
}

impl<T> Schema<T>
where
    T: AsRef<dyn Snapshot>,
{
    /// Creates a new schema from the database view.
    pub fn new(view: T) -> Self {
        Schema { view }
    }

    /// Returns `ProofMapIndex` with wallets.
    pub fn wallets(&self) -> ProofMapIndex<&T, PublicKey, Wallet> {
        ProofMapIndex::new(WALLET_TABLE, &self.view)
    }

    /// Returns history of the wallet with the given public key.
    pub fn wallet_history(&self, public_key: &PublicKey) -> ProofListIndex<&T, Hash> {
        ProofListIndex::new_in_family(WALLET_HISTORY_FAMILY, public_key, &self.view)
    }

    /// Returns wallet for the given public key.
    pub fn wallet(&self, pub_key: &PublicKey) -> Option<Wallet> {
        self.wallets().get(pub_key)
    }

    /// Returns `ProofMapIndex` with multisignature transfers.
    pub fn multisig_transfers(&self) -> ProofMapIndex<&T, Hash, MultisignatureTransfer> {
        ProofMapIndex::new(MULTISIG_TRANSFER_TABLE, &self.view)
    }

    /// Returns multisignature transfer for the given tx hash.
    pub fn multisig_transfer(&self, tx_hash: Hash) -> Option<MultisignatureTransfer> {
        self.multisig_transfers().get(&tx_hash)
    }

    /// Returns the state hash of cryptocurrency service.
    pub fn state_hash(&self) -> Vec<Hash> {
        vec![
            self.wallets().merkle_root(),
            self.multisig_transfers().merkle_root(),
        ]
    }
}

/// Implementation of mutable methods.
impl<'a> Schema<&'a mut Fork> {
    /// Returns mutable `ProofMapIndex` with wallets.
    pub fn wallets_mut(&mut self) -> ProofMapIndex<&mut Fork, PublicKey, Wallet> {
        ProofMapIndex::new(WALLET_TABLE, &mut self.view)
    }

    /// Returns history for the wallet by the given public key.
    pub fn wallet_history_mut(
        &mut self,
        public_key: &PublicKey,
    ) -> ProofListIndex<&mut Fork, Hash> {
        ProofListIndex::new_in_family(WALLET_HISTORY_FAMILY, public_key, &mut self.view)
    }

    /// Create new wallet and append first record to its history.
    pub fn create_wallet(&mut self, key: &PublicKey, name: &str, transaction: &Hash) {
        let wallet = {
            let mut history = self.wallet_history_mut(key);
            history.push(*transaction);
            let history_hash = history.merkle_root();
            Wallet::new(key, name, INITIAL_BALANCE, history.len(), &history_hash)
        };
        self.wallets_mut().put(key, wallet);
    }

    /// Update existing wallet after transaction.
    pub fn update_wallet(&mut self, wallet: Wallet, transaction: Hash) {
        let wallet = {
            let mut history = self.wallet_history_mut(&wallet.pub_key);
            history.push(transaction);
            let history_hash = history.merkle_root();

            wallet.update_history_hash(history_hash)
        };

        let key = wallet.pub_key;
        self.wallets_mut().put(&key, wallet);
    }

    /// Returns mutable `ProofMapIndex` with multisignature transactions.
    pub fn multisig_transfers_mut(
        &mut self,
    ) -> ProofMapIndex<&mut Fork, Hash, MultisignatureTransfer> {
        ProofMapIndex::new(MULTISIG_TRANSFER_TABLE, &mut self.view)
    }

    /// Put new pending MultisignatureTransfer into wallet.
    pub fn create_transfer_multisig(&mut self, transaction: Hash) {
        self.multisig_transfers_mut()
            .put(&transaction, MultisignatureTransfer::new());
    }

    /// Updates multisignature transfer.
    pub fn update_transfer_multisig(
        &mut self,
        transfer_tx: Hash,
        transfer: MultisignatureTransfer,
    ) {
        self.multisig_transfers_mut().put(&transfer_tx, transfer);
    }
}

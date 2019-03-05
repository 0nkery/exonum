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

use crate::{
    wallet::{PendingTransferMultisig, Wallet},
    INITIAL_BALANCE,
};

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
        ProofMapIndex::new("cryptocurrency.wallets", &self.view)
    }

    /// Returns history of the wallet with the given public key.
    pub fn wallet_history(&self, public_key: &PublicKey) -> ProofListIndex<&T, Hash> {
        ProofListIndex::new_in_family("cryptocurrency.wallet_history", public_key, &self.view)
    }

    /// Returns wallet for the given public key.
    pub fn wallet(&self, pub_key: &PublicKey) -> Option<Wallet> {
        self.wallets().get(pub_key)
    }

    /// Returns the state hash of cryptocurrency service.
    pub fn state_hash(&self) -> Vec<Hash> {
        vec![self.wallets().merkle_root()]
    }
}

/// Implementation of mutable methods.
impl<'a> Schema<&'a mut Fork> {
    /// Returns mutable `ProofMapIndex` with wallets.
    pub fn wallets_mut(&mut self) -> ProofMapIndex<&mut Fork, PublicKey, Wallet> {
        ProofMapIndex::new("cryptocurrency.wallets", &mut self.view)
    }

    /// Returns history for the wallet by the given public key.
    pub fn wallet_history_mut(
        &mut self,
        public_key: &PublicKey,
    ) -> ProofListIndex<&mut Fork, Hash> {
        ProofListIndex::new_in_family("cryptocurrency.wallet_history", public_key, &mut self.view)
    }

    /// Increase balance of the wallet and append new record to its history.
    ///
    /// Panics if there is no wallet with given public key.
    pub fn increase_wallet_balance(&mut self, wallet: Wallet, amount: u64, transaction: &Hash) {
        let wallet = {
            let mut history = self.wallet_history_mut(&wallet.pub_key);
            history.push(*transaction);
            let history_hash = history.merkle_root();
            let balance = wallet.balance;
            wallet.set_balance(balance + amount, history_hash)
        };
        self.wallets_mut().put(&wallet.pub_key, wallet.clone());
    }

    /// Decrease balance of the wallet and append new record to its history.
    ///
    /// Panics if there is no wallet with given public key.
    pub fn decrease_wallet_balance(&mut self, wallet: Wallet, amount: u64, transaction: &Hash) {
        let wallet = {
            let mut history = self.wallet_history_mut(&wallet.pub_key);
            history.push(*transaction);
            let history_hash = history.merkle_root();
            let balance = wallet.balance;
            wallet.set_balance(balance - amount, history_hash)
        };
        self.wallets_mut().put(&wallet.pub_key, wallet.clone());
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

    /// Put new pending MultisignatureTransfer into wallet.
    pub fn put_transfer_multisig(
        &mut self,
        wallet: Wallet,
        transfer_multisig: PendingTransferMultisig,
    ) {
        let wallet = {
            let mut history = self.wallet_history_mut(&wallet.pub_key);
            history.push(transfer_multisig.tx_hash);
            let history_hash = history.merkle_root();
            wallet.put_multisig_transfer(transfer_multisig, history_hash)
        };

        let pub_key = wallet.pub_key;

        self.wallets_mut().put(&pub_key, wallet);
    }

    /// Complete PendingTransferMultisig.
    pub fn complete_transfer_multisig(
        &mut self,
        wallet: Wallet,
        amount: u64,
        transfer_multisig: PendingTransferMultisig,
        transaction: Hash,
    ) {
        let wallet = {
            let mut history = self.wallet_history_mut(&wallet.pub_key);
            history.push(transaction);
            let history_hash = history.merkle_root();

            let balance = wallet.balance;

            wallet.complete_multisig_transfer(transfer_multisig, balance + amount, history_hash)
        };

        let pub_key = wallet.pub_key;

        self.wallets_mut().put(&pub_key, wallet);
    }

    /// Update PendingTransferMultisig.
    pub fn update_transfer_multisig(
        &mut self,
        wallet: Wallet,
        transfer_multisig: PendingTransferMultisig,
        transaction: Hash,
    ) {
        let wallet = {
            let mut history = self.wallet_history_mut(&wallet.pub_key);
            history.push(transaction);
            let history_hash = history.merkle_root();

            wallet.update_multisig_transfer(transfer_multisig, history_hash)
        };

        let pub_key = wallet.pub_key;

        self.wallets_mut().put(&pub_key, wallet);
    }

    /// Cancel PendingTransferMultisig.
    pub fn cancel_transfer_multisig(
        &mut self,
        wallet: Wallet,
        transfer_multisig: PendingTransferMultisig,
        transaction: Hash,
    ) {
        let wallet = {
            let mut history = self.wallet_history_mut(&wallet.pub_key);
            history.push(transaction);
            let history_hash = history.merkle_root();

            wallet.remove_multisig_transfer(transfer_multisig.tx_hash, history_hash)
        };

        let pub_key = wallet.pub_key;

        self.wallets_mut().put(&pub_key, wallet);
    }
}

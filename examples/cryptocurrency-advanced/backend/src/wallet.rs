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

//! Cryptocurrency wallet.

use exonum::crypto::{Hash, PublicKey};

use super::proto;

/// MultisignatureTransfer information stored in the database.
#[derive(Clone, Debug, ProtobufConvert, PartialEq)]
#[exonum(pb = "proto::PendingTransferMultisig", serde_pb_convert)]
pub struct PendingTransferMultisig {
    /// Hash of original TransferMultisig tx.
    pub tx_hash: Hash,
    /// Public keys of approvers approved this transfer.
    pub approved_by: Vec<PublicKey>,
}

impl PendingTransferMultisig {
    /// Create new MultisignatureTransfer.
    pub fn new(tx_hash: Hash) -> Self {
        Self {
            tx_hash,
            approved_by: Vec::new(),
        }
    }

    /// Approve the transfer.
    pub fn approve(self, approver: PublicKey) -> Self {
        let mut approved_by = self.approved_by;
        approved_by.push(approver);

        Self {
            approved_by,
            tx_hash: self.tx_hash,
        }
    }

    /// Shows if the transfer is approved by all required approvers.
    pub fn is_complete(&self, approvers: &[PublicKey]) -> bool {
        use std::collections::{hash_map::RandomState, HashSet};
        use std::iter::FromIterator;

        let approvers: HashSet<&PublicKey, RandomState> = HashSet::from_iter(approvers.iter());
        let approved_by = HashSet::from_iter(self.approved_by.iter());

        approved_by == approvers
    }
}

/// Wallet information stored in the database.
#[derive(Clone, Debug, ProtobufConvert)]
#[exonum(pb = "proto::Wallet", serde_pb_convert)]
pub struct Wallet {
    /// `PublicKey` of the wallet.
    pub pub_key: PublicKey,
    /// Name of the wallet.
    pub name: String,
    /// Current balance of the wallet.
    pub balance: u64,
    /// Length of the transactions history.
    pub history_len: u64,
    /// `Hash` of the transactions history.
    pub history_hash: Hash,
    /// List of pending multisignature transfers.
    pub pending_multisig_transfers: Vec<PendingTransferMultisig>,
}

impl Wallet {
    /// Create new Wallet.
    pub fn new(
        &pub_key: &PublicKey,
        name: &str,
        balance: u64,
        history_len: u64,
        &history_hash: &Hash,
    ) -> Self {
        Self {
            pub_key,
            name: name.to_owned(),
            balance,
            history_len,
            history_hash,
            pending_multisig_transfers: Vec::new(),
        }
    }

    /// Consumes and returns the wallet with updated balance.
    pub fn set_balance(self, balance: u64, history_hash: Hash) -> Self {
        Self {
            balance,
            history_hash,
            history_len: self.history_len + 1,

            pub_key: self.pub_key,
            name: self.name,
            pending_multisig_transfers: self.pending_multisig_transfers,
        }
    }

    /// Consumes and returns the wallet with updated pending multisignature transfers.
    pub fn put_multisig_transfer(
        self,
        transfer_multisig: PendingTransferMultisig,
        history_hash: Hash,
    ) -> Self {
        let mut pending_multisig_transfers = self.pending_multisig_transfers;
        pending_multisig_transfers.push(transfer_multisig);

        Self {
            pending_multisig_transfers,
            history_hash,
            history_len: self.history_len + 1,

            pub_key: self.pub_key,
            name: self.name,
            balance: self.balance,
        }
    }

    /// Consumes and returns the wallet with completed multisignature transfer.
    pub fn complete_multisig_transfer(
        self,
        transfer_multisig: PendingTransferMultisig,
        balance: u64,
        history_hash: Hash,
    ) -> Self {
        self.remove_multisig_transfer(transfer_multisig.tx_hash, history_hash)
            .set_balance(balance, history_hash)
    }

    /// Consumes and returns the wallet with updated multisignature transfer.
    pub fn update_multisig_transfer(
        self,
        transfer_multisig: PendingTransferMultisig,
        history_hash: Hash,
    ) -> Self {
        self.remove_multisig_transfer(transfer_multisig.tx_hash, history_hash)
            .put_multisig_transfer(transfer_multisig, history_hash)
    }

    /// Consumes and returns the wallet without pending multisignature transfer.
    pub fn remove_multisig_transfer(
        self,
        transfer_multisig_hash: Hash,
        history_hash: Hash,
    ) -> Self {
        let mut pending_multisig_transfers = self.pending_multisig_transfers;
        pending_multisig_transfers.retain(|t| t.tx_hash != transfer_multisig_hash);

        Self {
            pending_multisig_transfers,
            history_hash,
            history_len: self.history_len + 1,

            pub_key: self.pub_key,
            name: self.name,
            balance: self.balance,
        }
    }
}

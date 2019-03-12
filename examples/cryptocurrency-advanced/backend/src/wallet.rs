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
        }
    }

    /// Increase balance on wallet.
    pub fn increase_balance(self, amount: u64) -> Self {
        Self {
            balance: self.balance + amount,

            ..self
        }
    }

    /// Decrease balance on wallet.
    pub fn decrease_balance(self, amount: u64) -> Self {
        Self {
            balance: self.balance - amount,

            ..self
        }
    }

    /// Update history hash on wallet.
    pub fn update_history_hash(self, history_hash: Hash) -> Self {
        Self {
            history_hash,
            history_len: self.history_len + 1,

            ..self
        }
    }
}

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

//! Cryptocurrency transactions.

// Workaround for `failure` see https://github.com/rust-lang-nursery/failure/issues/223 and
// ECR-1771 for the details.
#![allow(bare_trait_objects)]

use std::collections::HashSet;

use exonum::{
    blockchain::{self, ExecutionError, ExecutionResult, Transaction, TransactionContext},
    crypto::{Hash, PublicKey, SecretKey},
    messages::{Message, RawTransaction, Signed},
};

use super::proto;
use crate::{schema::Schema, CRYPTOCURRENCY_SERVICE_ID};

/// Error codes emitted by wallet transactions during execution.
#[derive(Debug, Fail)]
#[repr(u8)]
pub enum Error {
    /// Wallet already exists.
    ///
    /// Can be emitted by `CreateWallet`.
    #[fail(display = "Wallet already exists")]
    WalletAlreadyExists = 0,

    /// Sender doesn't exist.
    ///
    /// Can be emitted by `Transfer` or `TransferMultisig`.
    #[fail(display = "Sender doesn't exist")]
    SenderNotFound = 1,

    /// Receiver doesn't exist.
    ///
    /// Can be emitted by `Transfer`, `TransferMultisig` or `Issue`.
    #[fail(display = "Receiver doesn't exist")]
    ReceiverNotFound = 2,

    /// Insufficient currency amount.
    ///
    /// Can be emitted by `Transfer` or `TransferMultisig`.
    #[fail(display = "Insufficient currency amount")]
    InsufficientCurrencyAmount = 3,

    /// Sender same as receiver.
    ///
    /// Can be emitted by `Transfer` or `TransferMultisig`.
    #[fail(display = "Sender same as receiver")]
    SenderSameAsReceiver = 4,

    /// Empty approvers list.
    ///
    /// Can be emitted by `TransferMultisig`.
    #[fail(display = "Empty approvers list")]
    EmptyApproversList = 5,

    /// Approvers list is too large.
    ///
    /// Can be emitted by `TransferMultisig`.
    #[fail(display = "Approvers list is too large")]
    ApproversListIsTooLarge = 6,

    /// Transaction does not exist.
    ///
    /// Can be emitted by `ApproveTransferMultisig`.
    #[fail(display = "Transaction does not exist")]
    TransactionDoesNotExist = 7,

    /// Referred transaction failed.
    ///
    /// Can be emitted by `ApproveTransferMultisig`.
    #[fail(display = "Referred transaction failed")]
    ReferredTransactionFailed = 8,

    /// Referred transaction is not `TransferMultisig`.
    ///
    /// Can be emitted by `ApproveTransferMultisig`.
    #[fail(display = "Referred transaction is not TransferMultisig")]
    ReferredTransactionIsNotTransferMultisig = 9,

    /// Approver is not on approvers list.
    ///
    /// Can be emitted by `ApproveTransferMultisig`.
    #[fail(display = "Approver is not on approvers list")]
    ApproverIsNotOnApproversList = 10,

    /// Transfer is rejected.
    ///
    /// Can be emitted by `ApproveTransferMultisig`.
    #[fail(display = "Transfer is rejected")]
    TransferIsRejected = 11,
}

impl From<Error> for ExecutionError {
    fn from(value: Error) -> ExecutionError {
        let description = format!("{}", value);
        ExecutionError::with_description(value as u8, description)
    }
}

/// Transfer `amount` of the currency from one wallet to another.
#[derive(Clone, Debug, ProtobufConvert)]
#[exonum(pb = "proto::Transfer", serde_pb_convert)]
pub struct Transfer {
    /// `PublicKey` of receiver's wallet.
    pub to: PublicKey,
    /// Amount of currency to transfer.
    pub amount: u64,
    /// Auxiliary number to guarantee [non-idempotence][idempotence] of transactions.
    ///
    /// [idempotence]: https://en.wikipedia.org/wiki/Idempotence
    pub seed: u64,
}

/// Transfer 'amount' of the currency from one wallet to another
/// after approval from all the 'approvers'.
#[derive(Debug, Clone, ProtobufConvert)]
#[exonum(pb = "proto::TransferMultisig", serde_pb_convert)]
pub struct TransferMultisig {
    /// `PublicKey` of receiver's wallet.
    pub to: PublicKey,
    /// Public keys of approvers.
    pub approvers: Vec<PublicKey>,
    /// Amount of currency to transfer.
    pub amount: u64,
    /// Auxiliary number to guarantee idempotence of transactions.
    pub seed: u64,
}

/// Approve multisignature transfer.
#[derive(Debug, Clone, ProtobufConvert)]
#[exonum(pb = "proto::ApproveTransferMultisig", serde_pb_convert)]
pub struct ApproveTransferMultisig {
    tx_hash: Hash,
}

/// Reject multisignature transfer.
#[derive(Debug, Clone, ProtobufConvert)]
#[exonum(pb = "proto::RejectTransferMultisig", serde_pb_convert)]
pub struct RejectTransferMultisig {
    tx_hash: Hash,
}

/// Issue `amount` of the currency to the `wallet`.
#[derive(Serialize, Deserialize, Clone, Debug, ProtobufConvert)]
#[exonum(pb = "proto::Issue")]
pub struct Issue {
    /// Issued amount of currency.
    pub amount: u64,
    /// Auxiliary number to guarantee [non-idempotence][idempotence] of transactions.
    ///
    /// [idempotence]: https://en.wikipedia.org/wiki/Idempotence
    pub seed: u64,
}

/// Create wallet with the given `name`.
#[derive(Serialize, Deserialize, Clone, Debug, ProtobufConvert)]
#[exonum(pb = "proto::CreateWallet")]
pub struct CreateWallet {
    /// Name of the new wallet.
    pub name: String,
}

/// Transaction group.
#[derive(Serialize, Deserialize, Clone, Debug, TransactionSet)]
pub enum WalletTransactions {
    /// Transfer tx.
    Transfer(Transfer),
    /// Issue tx.
    Issue(Issue),
    /// CreateWallet tx.
    CreateWallet(CreateWallet),
    /// TransferMultisig tx.
    TransferMultisig(TransferMultisig),
    /// ApproveTransferMultisig tx.
    ApproveTransferMultisig(ApproveTransferMultisig),
    /// RejectTransferMultisig tx.
    RejectTransferMultisig(RejectTransferMultisig),
}

impl CreateWallet {
    #[doc(hidden)]
    pub fn sign(name: &str, pk: &PublicKey, sk: &SecretKey) -> Signed<RawTransaction> {
        Message::sign_transaction(
            Self {
                name: name.to_owned(),
            },
            CRYPTOCURRENCY_SERVICE_ID,
            *pk,
            sk,
        )
    }
}

impl Transfer {
    #[doc(hidden)]
    pub fn sign(
        pk: &PublicKey,
        &to: &PublicKey,
        amount: u64,
        seed: u64,
        sk: &SecretKey,
    ) -> Signed<RawTransaction> {
        Message::sign_transaction(
            Self { to, amount, seed },
            CRYPTOCURRENCY_SERVICE_ID,
            *pk,
            sk,
        )
    }
}

impl TransferMultisig {
    #[doc(hidden)]
    pub fn sign(
        pk: PublicKey,
        sk: &SecretKey,
        to: PublicKey,
        // HashSet is used to guarantee an absense of duplicates.
        approvers: HashSet<PublicKey>,
        amount: u64,
        seed: u64,
    ) -> Signed<RawTransaction> {
        Message::sign_transaction(
            Self {
                to,
                approvers: approvers.into_iter().collect(),
                amount,
                seed,
            },
            CRYPTOCURRENCY_SERVICE_ID,
            pk,
            sk,
        )
    }
}

impl ApproveTransferMultisig {
    #[doc(hidden)]
    pub fn sign(pk: PublicKey, sk: &SecretKey, tx_hash: Hash) -> Signed<RawTransaction> {
        Message::sign_transaction(Self { tx_hash }, CRYPTOCURRENCY_SERVICE_ID, pk, sk)
    }
}

impl RejectTransferMultisig {
    #[doc(hidden)]
    pub fn sign(pk: PublicKey, sk: &SecretKey, tx_hash: Hash) -> Signed<RawTransaction> {
        Message::sign_transaction(Self { tx_hash }, CRYPTOCURRENCY_SERVICE_ID, pk, sk)
    }
}

impl Transaction for Transfer {
    fn execute(&self, mut context: TransactionContext) -> ExecutionResult {
        let from = &context.author();
        let hash = context.tx_hash();

        let mut schema = Schema::new(context.fork());

        let to = &self.to;
        let amount = self.amount;

        if from == to {
            Err(Error::SenderSameAsReceiver)?;
        }

        let sender = schema.wallet(from).ok_or(Error::SenderNotFound)?;
        let receiver = schema.wallet(to).ok_or(Error::ReceiverNotFound)?;

        if sender.balance < amount {
            Err(Error::InsufficientCurrencyAmount)?
        }

        schema.update_wallet(sender.decrease_balance(amount), hash);
        schema.update_wallet(receiver.increase_balance(amount), hash);

        Ok(())
    }
}

impl Transaction for Issue {
    fn execute(&self, mut context: TransactionContext) -> ExecutionResult {
        let pub_key = &context.author();
        let hash = context.tx_hash();

        let mut schema = Schema::new(context.fork());

        if let Some(wallet) = schema.wallet(pub_key) {
            schema.update_wallet(wallet.increase_balance(self.amount), hash);
            Ok(())
        } else {
            Err(Error::ReceiverNotFound)?
        }
    }
}

impl Transaction for CreateWallet {
    fn execute(&self, mut context: TransactionContext) -> ExecutionResult {
        let pub_key = &context.author();
        let hash = context.tx_hash();

        let mut schema = Schema::new(context.fork());

        if schema.wallet(pub_key).is_none() {
            let name = &self.name;
            schema.create_wallet(pub_key, name, &hash);
            Ok(())
        } else {
            Err(Error::WalletAlreadyExists)?
        }
    }
}

/// Some arbitrary constraint specifying how large approvers list can be.
pub const MAX_APPROVERS: usize = 5;

impl Transaction for TransferMultisig {
    fn execute(&self, mut context: TransactionContext) -> ExecutionResult {
        let from = context.author();
        let hash = context.tx_hash();

        let mut schema = Schema::new(context.fork());

        let to = self.to;
        let amount = self.amount;

        if from == to {
            return Err(Error::SenderSameAsReceiver.into());
        }

        let sender = schema.wallet(&from).ok_or(Error::SenderNotFound)?;
        let _receiver = schema.wallet(&to).ok_or(Error::ReceiverNotFound)?;

        if sender.balance < amount {
            return Err(Error::InsufficientCurrencyAmount.into());
        }

        let approvers: HashSet<PublicKey> = self.approvers.iter().cloned().collect();

        if approvers.is_empty() {
            return Err(Error::EmptyApproversList.into());
        }

        if approvers.len() > MAX_APPROVERS {
            return Err(Error::ApproversListIsTooLarge.into());
        }

        let sender = sender.decrease_balance(amount);

        schema.update_wallet(sender, hash);
        schema.create_transfer_multisig(hash);

        Ok(())
    }
}

impl Transaction for ApproveTransferMultisig {
    fn execute(&self, mut context: TransactionContext) -> ExecutionResult {
        use exonum::blockchain::TransactionSet;

        let original_transfer = {
            let blockchain = blockchain::Schema::new(context.fork());

            // Proof (in a sense) that tx was successful.
            blockchain
                .transaction_results()
                .get(&self.tx_hash)
                .ok_or(Error::TransactionDoesNotExist)?
                .0
                .map_err(|_err| Error::ReferredTransactionFailed)?;

            let raw_tx = blockchain
                .transactions()
                .get(&self.tx_hash)
                .ok_or(Error::TransactionDoesNotExist)?
                .payload()
                .clone();

            let tx = WalletTransactions::tx_from_raw(raw_tx)
                .map_err(|_err| Error::ReferredTransactionIsNotTransferMultisig)?;

            match tx {
                WalletTransactions::TransferMultisig(tx) => tx,
                _ => return Err(Error::ReferredTransactionIsNotTransferMultisig.into()),
            }
        };

        let approver = context.author();
        let tx_hash = context.tx_hash();
        let mut schema = Schema::new(context.fork());

        let wallet = schema
            .wallet(&original_transfer.to)
            // Highly unlikely (read as impossible) scenario but...
            .ok_or(Error::ReceiverNotFound)?;

        let transfer_in_question = schema
            .multisig_transfer(self.tx_hash)
            .ok_or(Error::TransactionDoesNotExist)?;

        if transfer_in_question.is_rejected() {
            return Err(Error::TransferIsRejected.into());
        }

        let approved_transfer = transfer_in_question
            .approve(approver, &original_transfer.approvers)
            .map_err(|_err| Error::ApproverIsNotOnApproversList)?;

        if approved_transfer.is_done() {
            let wallet = wallet.increase_balance(original_transfer.amount);
            schema.update_wallet(wallet, tx_hash);
        }

        schema.update_transfer_multisig(self.tx_hash, approved_transfer);

        Ok(())
    }
}

impl Transaction for RejectTransferMultisig {
    fn execute(&self, mut context: TransactionContext) -> ExecutionResult {
        use exonum::blockchain::TransactionSet;

        let (original_transfer, original_author) = {
            let blockchain = blockchain::Schema::new(context.fork());

            // Proof (in a sense) that tx was successful.
            blockchain
                .transaction_results()
                .get(&self.tx_hash)
                .ok_or(Error::TransactionDoesNotExist)?
                .0
                .map_err(|_err| Error::ReferredTransactionFailed)?;

            let signed = blockchain
                .transactions()
                .get(&self.tx_hash)
                .ok_or(Error::TransactionDoesNotExist)?;

            let raw_tx = signed.payload().clone();

            let tx = WalletTransactions::tx_from_raw(raw_tx)
                .map_err(|_err| Error::ReferredTransactionIsNotTransferMultisig)?;

            match tx {
                WalletTransactions::TransferMultisig(tx) => (tx, signed.author()),
                _ => return Err(Error::ReferredTransactionIsNotTransferMultisig.into()),
            }
        };

        let rejecter = context.author();
        let tx_hash = context.tx_hash();
        let mut schema = Schema::new(context.fork());

        let sender = schema
            .wallet(&original_author)
            .ok_or(Error::SenderNotFound)?;

        let transfer_in_question = schema
            .multisig_transfer(self.tx_hash)
            .ok_or(Error::TransactionDoesNotExist)?;

        let rejected_transfer = transfer_in_question
            .reject(rejecter, &original_transfer.approvers)
            .map_err(|_err| Error::ApproverIsNotOnApproversList)?;

        let sender = sender.increase_balance(original_transfer.amount);
        schema.update_wallet(sender, tx_hash);

        schema.update_transfer_multisig(self.tx_hash, rejected_transfer);

        Ok(())
    }
}

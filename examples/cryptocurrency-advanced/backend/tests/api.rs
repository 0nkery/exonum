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

//! These are tests concerning the API of the cryptocurrency service. See `tx_logic.rs`
//! for tests focused on the business logic of transactions.
//!
//! Note how API tests predominantly use `TestKitApi` to send transactions and make assertions
//! about the storage state.

#[macro_use]
extern crate serde_json;

use exonum::{
    api::{
        self,
        node::public::explorer::{TransactionQuery, TransactionResponse},
    },
    crypto::{self, Hash, PublicKey, SecretKey},
    helpers::Height,
    messages::{self, RawTransaction, Signed},
};
use exonum_testkit::{ApiKind, TestKit, TestKitApi, TestKitBuilder};

// Import data types used in tests from the crate where the service is defined.
use exonum_cryptocurrency_advanced::{
    api::{SimpleTransactionInfo, SimpleWalletInfo, WalletInfo, WalletQuery},
    transactions::{
        ApproveTransferMultisig, CreateWallet, RejectTransferMultisig, Transfer, TransferMultisig,
        MAX_APPROVERS,
    },
    wallet::Wallet,
    Service,
};

// Imports shared test constants.
use crate::constants::{ALICE_NAME, BOB_NAME};

mod constants;

/// Check that the wallet creation transaction works when invoked via API.
#[test]
fn test_create_wallet() {
    let (mut testkit, api) = create_testkit();
    // Create and send a transaction via API
    let (tx, _) = api.create_wallet(ALICE_NAME);
    testkit.create_block();
    api.assert_tx_status(tx.hash(), &json!({ "type": "success" }));

    // Check that the user indeed is persisted by the service.
    let wallet = api.get_wallet(tx.author()).unwrap();
    assert_eq!(wallet.pub_key, tx.author());
    assert_eq!(wallet.name, ALICE_NAME);
    assert_eq!(wallet.balance, 100);
}

/// Check that the transfer transaction works as intended.
#[test]
fn test_transfer() {
    // Create 2 wallets.
    let (mut testkit, api) = create_testkit();
    let (tx_alice, key_alice) = api.create_wallet(ALICE_NAME);
    let (tx_bob, _) = api.create_wallet(BOB_NAME);
    testkit.create_block();
    api.assert_tx_status(tx_alice.hash(), &json!({ "type": "success" }));
    api.assert_tx_status(tx_bob.hash(), &json!({ "type": "success" }));

    // Check that the initial Alice's and Bob's balances persisted by the service.
    let wallet = api.get_wallet(tx_alice.author()).unwrap();
    assert_eq!(wallet.balance, 100);
    let wallet = api.get_wallet(tx_bob.author()).unwrap();
    assert_eq!(wallet.balance, 100);

    // Transfer funds by invoking the corresponding API method.
    let tx = Transfer::sign(
        &tx_alice.author(),
        &tx_bob.author(),
        10, // transferred amount
        0,  // seed
        &key_alice,
    );
    api.transaction(&tx);
    testkit.create_block();
    api.assert_tx_status(tx.hash(), &json!({ "type": "success" }));

    // After the transfer transaction is included into a block, we may check new wallet
    // balances.
    let wallet = api.get_wallet(tx_alice.author()).unwrap();
    assert_eq!(wallet.balance, 90);
    let wallet = api.get_wallet(tx_bob.author()).unwrap();
    assert_eq!(wallet.balance, 110);
}

/// Check that a transfer from a non-existing wallet fails as expected.
#[test]
fn test_transfer_from_nonexisting_wallet() {
    let (mut testkit, api) = create_testkit();

    let (tx_alice, key_alice) = api.create_wallet(ALICE_NAME);
    let (tx_bob, _) = api.create_wallet(BOB_NAME);
    // Do not commit Alice's transaction, so Alice's wallet does not exist
    // when a transfer occurs.
    testkit.create_block_with_tx_hashes(&[tx_bob.hash()]);

    api.assert_no_wallet(tx_alice.author());
    let wallet = api.get_wallet(tx_bob.author()).unwrap();
    assert_eq!(wallet.balance, 100);

    let tx = Transfer::sign(
        &tx_alice.author(),
        &tx_bob.author(),
        10, // transfer amount
        0,  // seed
        &key_alice,
    );
    api.transaction(&tx);
    testkit.create_block_with_tx_hashes(&[tx.hash()]);
    api.assert_tx_status(
        tx.hash(),
        &json!({ "type": "error", "code": 1, "description": "Sender doesn't exist" }),
    );

    // Check that Bob's balance doesn't change.
    let wallet = api.get_wallet(tx_bob.author()).unwrap();
    assert_eq!(wallet.balance, 100);
}

/// Check that a transfer to a non-existing wallet fails as expected.
#[test]
fn test_transfer_to_nonexisting_wallet() {
    let (mut testkit, api) = create_testkit();

    let (tx_alice, key_alice) = api.create_wallet(ALICE_NAME);
    let (tx_bob, _) = api.create_wallet(BOB_NAME);
    // Do not commit Bob's transaction, so Bob's wallet does not exist
    // when a transfer occurs.
    testkit.create_block_with_tx_hashes(&[tx_alice.hash()]);

    let wallet = api.get_wallet(tx_alice.author()).unwrap();
    assert_eq!(wallet.balance, 100);
    api.assert_no_wallet(tx_bob.author());

    let tx = Transfer::sign(
        &tx_alice.author(),
        &tx_bob.author(),
        10, // transfer amount
        0,  // seed
        &key_alice,
    );
    api.transaction(&tx);
    testkit.create_block_with_tx_hashes(&[tx.hash()]);
    api.assert_tx_status(
        tx.hash(),
        &json!({ "type": "error", "code": 2, "description": "Receiver doesn't exist" }),
    );

    // Check that Alice's balance doesn't change.
    let wallet = api.get_wallet(tx_alice.author()).unwrap();
    assert_eq!(wallet.balance, 100);
}

/// Check that an overcharge does not lead to changes in sender's and receiver's balances.
#[test]
fn test_transfer_overcharge() {
    let (mut testkit, api) = create_testkit();

    let (tx_alice, key_alice) = api.create_wallet(ALICE_NAME);
    let (tx_bob, _) = api.create_wallet(BOB_NAME);
    testkit.create_block();

    // Transfer funds. The transfer amount (110) is more than Alice has (100).
    let tx = Transfer::sign(
        &tx_alice.author(),
        &tx_bob.author(),
        110, // transfer amount
        0,   // seed
        &key_alice,
    );
    api.transaction(&tx);
    testkit.create_block();
    api.assert_tx_status(
        tx.hash(),
        &json!({ "type": "error", "code": 3, "description": "Insufficient currency amount" }),
    );

    let wallet = api.get_wallet(tx_alice.author()).unwrap();
    assert_eq!(wallet.balance, 100);
    let wallet = api.get_wallet(tx_bob.author()).unwrap();
    assert_eq!(wallet.balance, 100);
}

#[test]
fn test_unknown_wallet_request() {
    let (_testkit, api) = create_testkit();

    // Transaction is sent by API, but isn't committed.
    let (tx, _) = api.create_wallet(ALICE_NAME);

    api.assert_no_wallet(tx.author());
}

#[test]
fn test_simple_wallet_info() {
    let (mut testkit, api) = create_testkit();

    // Create 2 wallets.
    let (tx_alice, key_alice) = api.create_wallet(ALICE_NAME);
    let (tx_bob, _key_bob) = api.create_wallet(BOB_NAME);
    testkit.create_block();
    api.assert_tx_status(tx_alice.hash(), &json!({ "type": "success" }));
    api.assert_tx_status(tx_bob.hash(), &json!({ "type": "success" }));

    // Check that the initial Alice's and Bob's balances persisted by the service.
    let wallet = api.get_wallet(tx_alice.author()).unwrap();
    assert_eq!(wallet.balance, 100);
    let wallet = api.get_wallet(tx_bob.author()).unwrap();
    assert_eq!(wallet.balance, 100);

    // Transfer funds by invoking the corresponding API method.
    let tx = Transfer::sign(
        &tx_alice.author(),
        &tx_bob.author(),
        10, // transferred amount
        0,  // seed
        &key_alice,
    );
    api.transaction(&tx);
    testkit.create_block();
    api.assert_tx_status(tx.hash(), &json!({ "type": "success" }));

    let response = api.simple_wallet_info(tx_alice.author()).unwrap();

    assert_eq!(
        vec![
            SimpleTransactionInfo {
                hash: tx_alice.hash(),
                height: Height(1),
            },
            SimpleTransactionInfo {
                hash: tx.hash(),
                height: Height(2),
            }
        ],
        response.transactions
    );

    let response = api.simple_wallet_info(tx_bob.author()).unwrap();

    assert_eq!(
        vec![
            SimpleTransactionInfo {
                hash: tx_bob.hash(),
                height: Height(1),
            },
            SimpleTransactionInfo {
                hash: tx.hash(),
                height: Height(2),
            }
        ],
        response.transactions
    );
}

#[test]
fn test_simple_wallet_info_on_unknown_public_key() {
    let (_testkit, api) = create_testkit();
    let (public_key, _private_key) = exonum_crypto::gen_keypair();
    let response = api.simple_wallet_info(public_key);

    assert!(response.is_err());

    if let Err(err) = response {
        println!("{}", err);
    }
}

/// Check that the multisignature transfer transaction works as intended.
#[test]
fn test_transfer_multisig() {
    let (mut testkit, api) = create_testkit();

    // Create 2 wallets.
    let (tx_alice, key_alice) = api.create_wallet(ALICE_NAME);
    let (tx_bob, _) = api.create_wallet(BOB_NAME);
    testkit.create_block();
    api.assert_tx_status(tx_alice.hash(), &json!({ "type": "success" }));
    api.assert_tx_status(tx_bob.hash(), &json!({ "type": "success" }));

    // Check that the initial Alice's and Bob's balances persisted by the service.
    let wallet = api.get_wallet(tx_alice.author()).unwrap();
    assert_eq!(wallet.balance, 100);
    let wallet = api.get_wallet(tx_bob.author()).unwrap();
    assert_eq!(wallet.balance, 100);

    // Create approvers.
    let (carol_public_key, carol_private_key) = exonum_crypto::gen_keypair();
    let (dave_public_key, dave_private_key) = exonum_crypto::gen_keypair();

    // Transfer funds by invoking the corresponding API method.
    let tx = TransferMultisig::sign(
        tx_alice.author(),
        &key_alice,
        tx_bob.author(),
        [carol_public_key, dave_public_key]
            .iter()
            .cloned()
            .collect(),
        10, // transferred amount
        0,  // seed
    );
    api.transaction(&tx);
    testkit.create_block();
    api.assert_tx_status(tx.hash(), &json!({ "type": "success" }));

    // Approve transfer.

    let tx_carol = ApproveTransferMultisig::sign(carol_public_key, &carol_private_key, tx.hash());
    api.transaction(&tx_carol);
    testkit.create_block();
    api.assert_tx_status(tx_carol.hash(), &json!({ "type": "success" }));

    let tx_dave = ApproveTransferMultisig::sign(dave_public_key, &dave_private_key, tx.hash());
    api.transaction(&tx_dave);
    testkit.create_block();
    api.assert_tx_status(tx_dave.hash(), &json!({ "type": "success" }));

    // After the multisignature transfer transaction is approved,
    // we may check new wallet balances.
    let wallet = api.get_wallet(tx_alice.author()).unwrap();
    assert_eq!(wallet.balance, 90);
    let wallet = api.get_wallet(tx_bob.author()).unwrap();
    assert_eq!(wallet.balance, 110);
}

/// Check that a multisignature transfer from a non-existing wallet fails as expected.
#[test]
fn test_transfer_multisig_from_nonexisting_wallet() {
    let (mut testkit, api) = create_testkit();

    let (tx_alice, key_alice) = api.create_wallet(ALICE_NAME);
    let (tx_bob, _) = api.create_wallet(BOB_NAME);
    // Do not commit Alice's transaction, so Alice's wallet does not exist
    // when a transfer occurs.
    testkit.create_block_with_tx_hashes(&[tx_bob.hash()]);

    api.assert_no_wallet(tx_alice.author());
    let wallet = api.get_wallet(tx_bob.author()).unwrap();
    assert_eq!(wallet.balance, 100);

    // Create approvers.
    let (carol_public_key, _carol_private_key) = exonum_crypto::gen_keypair();
    let (dave_public_key, _dave_private_key) = exonum_crypto::gen_keypair();

    // Transfer funds by invoking the corresponding API method.
    let tx = TransferMultisig::sign(
        tx_alice.author(),
        &key_alice,
        tx_bob.author(),
        [carol_public_key, dave_public_key]
            .iter()
            .cloned()
            .collect(),
        10, // transferred amount
        0,  // seed
    );
    api.transaction(&tx);
    testkit.create_block_with_tx_hashes(&[tx.hash()]);
    api.assert_tx_status(
        tx.hash(),
        &json!({ "type": "error", "code": 1, "description": "Sender doesn't exist" }),
    );

    // Check that Bob's balance doesn't change.
    let wallet = api.get_wallet(tx_bob.author()).unwrap();
    assert_eq!(wallet.balance, 100);
}

/// Check that a multisignature transfer to a non-existing wallet fails as expected.
#[test]
fn test_transfer_multisig_to_nonexisting_wallet() {
    let (mut testkit, api) = create_testkit();

    let (tx_alice, key_alice) = api.create_wallet(ALICE_NAME);
    let (tx_bob, _) = api.create_wallet(BOB_NAME);
    // Do not commit Bob's transaction, so Bob's wallet does not exist
    // when a transfer occurs.
    testkit.create_block_with_tx_hashes(&[tx_alice.hash()]);

    let wallet = api.get_wallet(tx_alice.author()).unwrap();
    assert_eq!(wallet.balance, 100);
    api.assert_no_wallet(tx_bob.author());

    // Create approvers.
    let (carol_public_key, _carol_private_key) = exonum_crypto::gen_keypair();
    let (dave_public_key, _dave_private_key) = exonum_crypto::gen_keypair();

    // Transfer funds by invoking the corresponding API method.
    let tx = TransferMultisig::sign(
        tx_alice.author(),
        &key_alice,
        tx_bob.author(),
        [carol_public_key, dave_public_key]
            .iter()
            .cloned()
            .collect(),
        10, // transferred amount
        0,  // seed
    );
    api.transaction(&tx);
    testkit.create_block_with_tx_hashes(&[tx.hash()]);
    api.assert_tx_status(
        tx.hash(),
        &json!({ "type": "error", "code": 2, "description": "Receiver doesn't exist" }),
    );

    // Check that Alice's balance doesn't change.
    let wallet = api.get_wallet(tx_alice.author()).unwrap();
    assert_eq!(wallet.balance, 100);
}

/// Check that an overcharge does not lead to changes in sender's and receiver's balances.
#[test]
fn test_transfer_multisig_overcharge() {
    let (mut testkit, api) = create_testkit();

    let (tx_alice, key_alice) = api.create_wallet(ALICE_NAME);
    let (tx_bob, _) = api.create_wallet(BOB_NAME);
    testkit.create_block();

    // Create approvers.
    let (carol_public_key, _carol_private_key) = exonum_crypto::gen_keypair();
    let (dave_public_key, _dave_private_key) = exonum_crypto::gen_keypair();

    // Transfer funds by invoking the corresponding API method.
    let tx = TransferMultisig::sign(
        tx_alice.author(),
        &key_alice,
        tx_bob.author(),
        [carol_public_key, dave_public_key]
            .iter()
            .cloned()
            .collect(),
        110, // transferred amount
        0,   // seed
    );
    api.transaction(&tx);
    testkit.create_block();
    api.assert_tx_status(
        tx.hash(),
        &json!({ "type": "error", "code": 3, "description": "Insufficient currency amount" }),
    );

    let wallet = api.get_wallet(tx_alice.author()).unwrap();
    assert_eq!(wallet.balance, 100);
    let wallet = api.get_wallet(tx_bob.author()).unwrap();
    assert_eq!(wallet.balance, 100);
}

#[test]
fn test_transfer_multisig_same_sender_and_receiver() {
    let (mut testkit, api) = create_testkit();

    let (tx_alice, key_alice) = api.create_wallet(ALICE_NAME);
    testkit.create_block();

    // Create approvers.
    let (carol_public_key, _carol_private_key) = exonum_crypto::gen_keypair();
    let (dave_public_key, _dave_private_key) = exonum_crypto::gen_keypair();

    // Transfer funds by invoking the corresponding API method.
    let tx = TransferMultisig::sign(
        tx_alice.author(),
        &key_alice,
        // Specify Alice as sender and receiver.
        tx_alice.author(),
        [carol_public_key, dave_public_key]
            .iter()
            .cloned()
            .collect(),
        10, // transferred amount
        0,  // seed
    );
    api.transaction(&tx);
    testkit.create_block();
    api.assert_tx_status(
        tx.hash(),
        &json!({ "type": "error", "code": 4, "description": "Sender same as receiver" }),
    );

    let wallet = api.get_wallet(tx_alice.author()).unwrap();
    assert_eq!(wallet.balance, 100);
}

#[test]
fn test_transfer_multisig_empty_approvers_list() {
    let (mut testkit, api) = create_testkit();

    let (tx_alice, key_alice) = api.create_wallet(ALICE_NAME);
    let (tx_bob, _) = api.create_wallet(BOB_NAME);
    testkit.create_block();

    // Transfer funds by invoking the corresponding API method.
    let tx = TransferMultisig::sign(
        tx_alice.author(),
        &key_alice,
        tx_bob.author(),
        // Send empty approvers list.
        [].iter().cloned().collect(),
        10, // transferred amount
        0,  // seed
    );
    api.transaction(&tx);
    testkit.create_block();
    api.assert_tx_status(
        tx.hash(),
        &json!({ "type": "error", "code": 5, "description": "Empty approvers list" }),
    );

    let wallet = api.get_wallet(tx_alice.author()).unwrap();
    assert_eq!(wallet.balance, 100);
    let wallet = api.get_wallet(tx_bob.author()).unwrap();
    assert_eq!(wallet.balance, 100);
}

#[test]
fn test_transfer_multisig_too_large_approvers_list() {
    let (mut testkit, api) = create_testkit();

    let (tx_alice, key_alice) = api.create_wallet(ALICE_NAME);
    let (tx_bob, _) = api.create_wallet(BOB_NAME);
    testkit.create_block();

    let mut approvers = Vec::new();

    for _ in 0..=MAX_APPROVERS {
        let (public_key, _private_key) = exonum_crypto::gen_keypair();
        approvers.push(public_key);
    }

    // Transfer funds by invoking the corresponding API method.
    let tx = TransferMultisig::sign(
        tx_alice.author(),
        &key_alice,
        tx_bob.author(),
        approvers.iter().cloned().collect(),
        10, // transferred amount
        0,  // seed
    );
    api.transaction(&tx);
    testkit.create_block();
    api.assert_tx_status(
        tx.hash(),
        &json!({ "type": "error", "code": 6, "description": "Approvers list is too large" }),
    );

    let wallet = api.get_wallet(tx_alice.author()).unwrap();
    assert_eq!(wallet.balance, 100);
    let wallet = api.get_wallet(tx_bob.author()).unwrap();
    assert_eq!(wallet.balance, 100);
}

#[test]
fn test_transfer_multisig_approve_non_existent_tx() {
    let (mut testkit, api) = create_testkit();

    let (tx_alice, key_alice) = api.create_wallet(ALICE_NAME);
    let (tx_bob, _) = api.create_wallet(BOB_NAME);
    testkit.create_block();

    // Create approvers.
    let (carol_public_key, carol_private_key) = exonum_crypto::gen_keypair();
    let (dave_public_key, _dave_private_key) = exonum_crypto::gen_keypair();

    // Transfer funds by invoking the corresponding API method.
    let tx = TransferMultisig::sign(
        tx_alice.author(),
        &key_alice,
        tx_bob.author(),
        [carol_public_key, dave_public_key]
            .iter()
            .cloned()
            .collect(),
        10, // transferred amount
        0,  // seed
    );
    api.transaction(&tx);
    // Don't create a block so tx will not exist.

    let tx_carol = ApproveTransferMultisig::sign(carol_public_key, &carol_private_key, tx.hash());
    api.transaction(&tx_carol);
    // Create block with Carol's tx only.
    testkit.create_block_with_tx_hashes(&[tx_carol.hash()]);
    api.assert_tx_status(
        tx_carol.hash(),
        &json!({ "type": "error", "code": 7, "description": "Transaction does not exist" }),
    );

    let wallet = api.get_wallet(tx_alice.author()).unwrap();
    assert_eq!(wallet.balance, 100);
    let wallet = api.get_wallet(tx_bob.author()).unwrap();
    assert_eq!(wallet.balance, 100);
}

#[test]
fn test_transfer_multisig_approve_on_failed_tx() {
    let (mut testkit, api) = create_testkit();

    let (tx_alice, key_alice) = api.create_wallet(ALICE_NAME);
    let (tx_bob, _) = api.create_wallet(BOB_NAME);
    testkit.create_block();

    // Create approvers.
    let (carol_public_key, carol_private_key) = exonum_crypto::gen_keypair();
    let (dave_public_key, _dave_private_key) = exonum_crypto::gen_keypair();

    // Transfer funds by invoking the corresponding API method.
    let tx = TransferMultisig::sign(
        tx_alice.author(),
        &key_alice,
        tx_bob.author(),
        [carol_public_key, dave_public_key]
            .iter()
            .cloned()
            .collect(),
        // Should fail due to overcharge.
        110, // transferred amount
        0,   // seed
    );
    api.transaction(&tx);
    testkit.create_block();

    let tx_carol = ApproveTransferMultisig::sign(carol_public_key, &carol_private_key, tx.hash());
    api.transaction(&tx_carol);
    testkit.create_block();
    api.assert_tx_status(
        tx_carol.hash(),
        &json!({ "type": "error", "code": 8, "description": "Referred transaction failed" }),
    );

    let wallet = api.get_wallet(tx_alice.author()).unwrap();
    assert_eq!(wallet.balance, 100);
    let wallet = api.get_wallet(tx_bob.author()).unwrap();
    assert_eq!(wallet.balance, 100);
}

#[test]
fn test_transfer_multisig_approve_on_some_non_related_tx() {
    let (mut testkit, api) = create_testkit();

    let (tx_alice, _key_alice) = api.create_wallet(ALICE_NAME);
    testkit.create_block();

    // Create approvers.
    let (carol_public_key, carol_private_key) = exonum_crypto::gen_keypair();

    let tx_carol =
        ApproveTransferMultisig::sign(carol_public_key, &carol_private_key, tx_alice.hash());
    api.transaction(&tx_carol);
    testkit.create_block();
    api.assert_tx_status(
        tx_carol.hash(),
        &json!({ "type": "error", "code": 9, "description": "Referred transaction is not TransferMultisig" }),
    );

    let wallet = api.get_wallet(tx_alice.author()).unwrap();
    assert_eq!(wallet.balance, 100);
}

#[test]
fn test_transfer_multisig_approver_non_eligible_to_approve() {
    let (mut testkit, api) = create_testkit();

    let (tx_alice, key_alice) = api.create_wallet(ALICE_NAME);
    let (tx_bob, _) = api.create_wallet(BOB_NAME);
    testkit.create_block();

    // Create approvers.
    let (carol_public_key, _carol_private_key) = exonum_crypto::gen_keypair();
    let (dave_public_key, dave_private_key) = exonum_crypto::gen_keypair();

    // Transfer funds by invoking the corresponding API method.
    let tx = TransferMultisig::sign(
        tx_alice.author(),
        &key_alice,
        tx_bob.author(),
        // Only Carol is allowed to approve the transfer.
        [carol_public_key].iter().cloned().collect(),
        10, // transferred amount
        0,  // seed
    );
    api.transaction(&tx);
    testkit.create_block();

    let tx_dave = ApproveTransferMultisig::sign(dave_public_key, &dave_private_key, tx.hash());
    api.transaction(&tx_dave);
    testkit.create_block();
    api.assert_tx_status(
        tx_dave.hash(),
        &json!({ "type": "error", "code": 10, "description": "Approver is not on approvers list" }),
    );

    let wallet = api.get_wallet(tx_alice.author()).unwrap();
    assert_eq!(wallet.balance, 90);
    let wallet = api.get_wallet(tx_bob.author()).unwrap();
    assert_eq!(wallet.balance, 100);
}

#[test]
fn test_transfer_multisig_reject() {
    let (mut testkit, api) = create_testkit();

    let (tx_alice, key_alice) = api.create_wallet(ALICE_NAME);
    let (tx_bob, _) = api.create_wallet(BOB_NAME);
    testkit.create_block();

    // Create approvers.
    let (carol_public_key, carol_private_key) = exonum_crypto::gen_keypair();
    let (dave_public_key, dave_private_key) = exonum_crypto::gen_keypair();

    // Transfer funds by invoking the corresponding API method.
    let tx = TransferMultisig::sign(
        tx_alice.author(),
        &key_alice,
        tx_bob.author(),
        [carol_public_key, dave_public_key]
            .iter()
            .cloned()
            .collect(),
        10, // transferred amount
        0,  // seed
    );
    api.transaction(&tx);
    testkit.create_block();

    let tx_dave = ApproveTransferMultisig::sign(dave_public_key, &dave_private_key, tx.hash());
    api.transaction(&tx_dave);
    testkit.create_block();
    api.assert_tx_status(tx_dave.hash(), &json!({ "type": "success" }));

    // Carol decides to reject the transfer.
    let tx_carol = RejectTransferMultisig::sign(carol_public_key, &carol_private_key, tx.hash());
    api.transaction(&tx_carol);
    testkit.create_block();
    api.assert_tx_status(tx_carol.hash(), &json!({ "type": "success" }));

    let wallet = api.get_wallet(tx_alice.author()).unwrap();
    assert_eq!(wallet.balance, 100);
    let wallet = api.get_wallet(tx_bob.author()).unwrap();
    assert_eq!(wallet.balance, 100);
}

/// Wrapper for the cryptocurrency service API allowing to easily use it
/// (compared to `TestKitApi` calls).
struct CryptocurrencyApi {
    pub inner: TestKitApi,
}

impl CryptocurrencyApi {
    /// Generates a wallet creation transaction with a random key pair, sends it over HTTP,
    /// and checks the synchronous result (i.e., the hash of the transaction returned
    /// within the response).
    /// Note that the transaction is not immediately added to the blockchain, but rather is put
    /// to the pool of unconfirmed transactions.
    fn create_wallet(&self, name: &str) -> (Signed<RawTransaction>, SecretKey) {
        let (pubkey, key) = crypto::gen_keypair();
        // Create a pre-signed transaction
        let tx = CreateWallet::sign(name, &pubkey, &key);

        let data = messages::to_hex_string(&tx);
        let tx_info: TransactionResponse = self
            .inner
            .public(ApiKind::Explorer)
            .query(&json!({ "tx_body": data }))
            .post("v1/transactions")
            .unwrap();
        assert_eq!(tx_info.tx_hash, tx.hash());
        (tx, key)
    }

    /// Sends a transfer transaction over HTTP and checks the synchronous result.
    fn transaction(&self, tx: &Signed<RawTransaction>) {
        let data = messages::to_hex_string(&tx);
        let tx_info: TransactionResponse = self
            .inner
            .public(ApiKind::Explorer)
            .query(&json!({ "tx_body": data }))
            .post("v1/transactions")
            .unwrap();
        assert_eq!(tx_info.tx_hash, tx.hash());
    }

    fn get_wallet(&self, pub_key: PublicKey) -> Option<Wallet> {
        let wallet_info = self
            .inner
            .public(ApiKind::Service("cryptocurrency"))
            .query(&WalletQuery { pub_key })
            .get::<WalletInfo>("v1/wallets/info")
            .unwrap();

        let to_wallet = wallet_info.wallet_proof.to_wallet.check().unwrap();
        let wallet = to_wallet
            .all_entries()
            .find(|(ref k, _)| **k == pub_key)
            .and_then(|tuple| tuple.1)
            .cloned();

        wallet
    }

    fn simple_wallet_info(&self, pub_key: PublicKey) -> api::Result<SimpleWalletInfo> {
        self.inner
            .public(ApiKind::Service("cryptocurrency"))
            .query(&WalletQuery { pub_key })
            .get::<SimpleWalletInfo>("v1/wallets/info/simple")
    }

    /// Asserts that a wallet with the specified public key is not known to the blockchain.
    fn assert_no_wallet(&self, pub_key: PublicKey) {
        let wallet_info: WalletInfo = self
            .inner
            .public(ApiKind::Service("cryptocurrency"))
            .query(&WalletQuery { pub_key })
            .get("v1/wallets/info")
            .unwrap();

        let to_wallet = wallet_info.wallet_proof.to_wallet.check().unwrap();
        assert!(to_wallet.missing_keys().find(|v| **v == pub_key).is_some())
    }

    /// Asserts that the transaction with the given hash has a specified status.
    fn assert_tx_status(&self, tx_hash: Hash, expected_status: &serde_json::Value) {
        let info: serde_json::Value = self
            .inner
            .public(ApiKind::Explorer)
            .query(&TransactionQuery::new(tx_hash))
            .get("v1/transactions")
            .unwrap();

        if let serde_json::Value::Object(mut info) = info {
            let tx_status = info.remove("status").unwrap();
            assert_eq!(tx_status, *expected_status);
        } else {
            panic!("Invalid transaction info format, object expected");
        }
    }
}

/// Creates a testkit together with the API wrapper defined above.
fn create_testkit() -> (TestKit, CryptocurrencyApi) {
    let testkit = TestKitBuilder::validator().with_service(Service).create();
    let api = CryptocurrencyApi {
        inner: testkit.api(),
    };
    (testkit, api)
}

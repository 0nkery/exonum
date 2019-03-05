# Docs

There were 2 additions to API in this update:
  - simple wallet info endpoint
  - multisignature funds transfer

--------------

## Simple wallet info endpoint

### URL

Endpoint is accessible via this URL:

```
/api/services/cryptocurrency/v1/wallets/info
```

### Query parameters

Name | Type | Description
---- | ---- | -----------
`pub_key` | String | Public key of interesting wallet

### Response

#### Errors

Code | Reason
---- | -----------
404  | Requested wallet is not found

#### On success

Returns a list of Transaction objects.

#### Transaction Object

Field | Type | Description
----- | ---- | -----------
`hash` | String | Hash of committed transaction on a given wallet.
`height` | Int | Block height at which transaction has been committed.

#### Example

```json
[
  {
    "hash": "haaaash1",
    "height": 1
  },
  {
    "hash": "haaaaaaash2",
    "height": 2
  }
]
```

----------

## Multisignature funds transfer

Allows to transfer funds from one wallet to another after
approval from specified approvers identified by public keys.

The whole process boils down to 3 transactions: `TransferMultisig`, `ApproveTransferMultisig` and `RejectTransferMultisig`. The first one is used to initiate transfer,
the second one is used to approve transfer by
particular participant listed as approver in initial transfer
proposal. When all the approvers gave their approval, transfer is done
and receiver gets the money. The third one allows to reject transfer and sender gets the money back.

### TransferMultisig

Initiates the process of multisignature transfer. Later transaction
should use its hash to approve/reject transfer.

#### Fields

Name | Type | Description
---- | ---- | -----------
to | Public key | Public key of receiving wallet
approvers | List of Public key | List of public keys of participants expected to approve/reject transfer (max length of list = 5)
amount | Int | Amount of currency being transferred

#### Errors

Errors possible during transaction execution:

Code | Description
---- | -----------
1 | Sender is not found
2 | Receiver is not found
3 | Sender has insufficient currency amount
4 | Sender same as receiver
5 | Empty `approvers`
6 | `approvers` is too large (>5)

### ApproveTransferMultisig

Approve the transfer. If this is the last required approval,
transfer is done and receiver gets the money.

#### Fields

Name | Type | Description
---- | ---- | -----------
tx_hash | Hash | Hash of TransferMultisig tx you want to approve

#### Errors

Errors possible during transaction execution:

Code | Description
---- | -----------
7 | Transfer does not exist
8 | Referred transfer failed
9 | Wrong type of referred tx (should be TransferMultisig)
10 | Tx author is not allowed to approve transfer

### RejectTransferMultisig

Reject the transfer. Other pending approvals discarded, transfer
aborted and sender gets the money back.

#### Fields

Name | Type | Description
---- | ---- | -----------
tx_hash | Hash | Hash of TransferMultisig tx you want to approve

#### Errors

Errors possible during transaction execution:

Code | Description
---- | -----------
7 | Transfer does not exist
8 | Referred transfer failed
9 | Wrong type of referred tx (should be TransferMultisig)
10 | Tx author is not allowed to approve transfer

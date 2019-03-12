//! Multisignature transfer.

use exonum::{crypto::PublicKey, proto::ProtobufConvert};

use super::proto::{self, MultisignatureTransfer_State};

/// State of multisignature transfer.
#[derive(Debug, Clone, PartialEq)]
#[repr(u8)]
pub enum State {
    /// Transfer is in process.
    InProcess = 0,
    /// Transfer was rejected by one of approvers.
    Rejected = 1,
    /// Transfer was approved by all the approvers.
    Done = 2,
}

impl ProtobufConvert for State {
    type ProtoStruct = MultisignatureTransfer_State;

    fn to_pb(&self) -> Self::ProtoStruct {
        match self {
            State::InProcess => MultisignatureTransfer_State::IN_PROCESS,
            State::Rejected => MultisignatureTransfer_State::REJECTED,
            State::Done => MultisignatureTransfer_State::DONE,
        }
    }

    fn from_pb(pb: Self::ProtoStruct) -> Result<Self, failure::Error> {
        match pb {
            MultisignatureTransfer_State::IN_PROCESS => Ok(State::InProcess),
            MultisignatureTransfer_State::REJECTED => Ok(State::Rejected),
            MultisignatureTransfer_State::DONE => Ok(State::Done),
        }
    }
}

/// MultisignatureTransfer information stored in the database.
#[derive(Clone, Debug, ProtobufConvert, PartialEq)]
#[exonum(pb = "proto::MultisignatureTransfer", serde_pb_convert)]
pub struct MultisignatureTransfer {
    /// Public keys of approvers approved this transfer.
    pub approved_by: Vec<PublicKey>,
    /// State of transfer.
    pub state: State,
}

impl Default for MultisignatureTransfer {
    fn default() -> Self {
        Self {
            approved_by: Vec::new(),
            state: State::InProcess,
        }
    }
}

impl MultisignatureTransfer {
    /// Create new MultisignatureTransfer.
    pub fn new() -> Self {
        Self::default()
    }

    /// Approve the transfer.
    ///
    /// Fails if approver is not on approver's list.
    pub fn approve(self, approver: PublicKey, approvers: &[PublicKey]) -> Result<Self, Self> {
        let in_approvers = approvers.iter().find(|a| **a == approver);

        if in_approvers.is_some() {
            let mut approved_by = self.approved_by;
            approved_by.push(approver);

            let approved = Self {
                approved_by,
                ..self
            };

            let state = if approved.is_complete(approvers) {
                State::Done
            } else {
                State::InProcess
            };

            Ok(Self { state, ..approved })
        } else {
            Err(self)
        }
    }

    /// Shows if the transfer is done.
    pub fn is_done(&self) -> bool {
        self.state == State::Done
    }

    /// Shows if the transfer is rejected.
    pub fn is_rejected(&self) -> bool {
        self.state == State::Rejected
    }

    /// Reject the transfer.
    ///
    /// Fails if approver is not on approver's list.
    pub fn reject(self, rejecter: PublicKey, approvers: &[PublicKey]) -> Result<Self, Self> {
        let in_approvers = approvers.iter().find(|a| **a == rejecter);

        if in_approvers.is_some() {
            Ok(Self {
                state: State::Rejected,
                ..self
            })
        } else {
            Err(self)
        }
    }

    /// Shows if the transfer is approved by all required approvers.
    fn is_complete(&self, approvers: &[PublicKey]) -> bool {
        use std::collections::{hash_map::RandomState, HashSet};
        use std::iter::FromIterator;

        let approvers: HashSet<&PublicKey, RandomState> = HashSet::from_iter(approvers.iter());
        let approved_by = HashSet::from_iter(self.approved_by.iter());

        approved_by == approvers
    }
}

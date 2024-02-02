pub use self::misbehaviour::{MisbehaviourProxyMessage, PrevState};
pub use self::update_client::{aggregate_messages, EmittedState, UpdateStateProxyMessage};
pub use self::verify_membership::{CommitmentPrefix, VerifyMembershipProxyMessage};
use crate::encoder::EthABIEncoder;
use crate::prelude::*;
use crate::Error;
use alloy_sol_types::{sol, SolValue};
use core::fmt::Display;
use serde::{Deserialize, Serialize};
mod misbehaviour;
mod update_client;
mod verify_membership;

pub const MESSAGE_SCHEMA_VERSION: u16 = 1;
pub const MESSAGE_HEADER_SIZE: usize = 32;

pub const MESSAGE_TYPE_UPDATE_STATE: u16 = 1;
pub const MESSAGE_TYPE_STATE: u16 = 2;
pub const MESSAGE_TYPE_MISBEHAVIOUR: u16 = 3;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ProxyMessage {
    UpdateState(UpdateStateProxyMessage),
    VerifyMembership(VerifyMembershipProxyMessage),
    Misbehaviour(MisbehaviourProxyMessage),
}

impl ProxyMessage {
    pub fn to_bytes(self) -> Vec<u8> {
        self.ethabi_encode()
    }

    pub fn from_bytes(bz: &[u8]) -> Result<Self, Error> {
        Self::ethabi_decode(bz)
    }

    // MSB first
    // 0-1:  version
    // 2-3:  message type
    // 4-31: reserved
    pub fn header(&self) -> [u8; MESSAGE_HEADER_SIZE] {
        let mut header = [0u8; MESSAGE_HEADER_SIZE];
        header[0..=1].copy_from_slice(&MESSAGE_SCHEMA_VERSION.to_be_bytes());
        header[2..=3].copy_from_slice(&self.message_type().to_be_bytes());
        header
    }

    pub fn message_type(&self) -> u16 {
        match self {
            Self::UpdateState(_) => MESSAGE_TYPE_UPDATE_STATE,
            Self::VerifyMembership(_) => MESSAGE_TYPE_STATE,
            Self::Misbehaviour(_) => MESSAGE_TYPE_MISBEHAVIOUR,
        }
    }

    pub fn validate(&self) -> Result<(), Error> {
        match self {
            Self::UpdateState(c) => c.validate(),
            Self::VerifyMembership(c) => c.validate(),
            Self::Misbehaviour(c) => c.validate(),
        }
    }
}

impl Display for ProxyMessage {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::UpdateState(c) => write!(f, "{}", c),
            Self::VerifyMembership(c) => write!(f, "{}", c),
            Self::Misbehaviour(c) => write!(f, "{}", c),
        }
    }
}

impl TryFrom<ProxyMessage> for UpdateStateProxyMessage {
    type Error = Error;
    fn try_from(value: ProxyMessage) -> Result<Self, Self::Error> {
        match value {
            ProxyMessage::UpdateState(m) => Ok(m),
            _ => Err(Error::unexpected_message_type(
                MESSAGE_TYPE_UPDATE_STATE,
                value.message_type(),
            )),
        }
    }
}

impl TryFrom<ProxyMessage> for VerifyMembershipProxyMessage {
    type Error = Error;
    fn try_from(value: ProxyMessage) -> Result<Self, Self::Error> {
        match value {
            ProxyMessage::VerifyMembership(m) => Ok(m),
            _ => Err(Error::unexpected_message_type(
                MESSAGE_TYPE_STATE,
                value.message_type(),
            )),
        }
    }
}

impl TryFrom<ProxyMessage> for MisbehaviourProxyMessage {
    type Error = Error;
    fn try_from(value: ProxyMessage) -> Result<Self, Self::Error> {
        match value {
            ProxyMessage::Misbehaviour(m) => Ok(m),
            _ => Err(Error::unexpected_message_type(
                MESSAGE_TYPE_MISBEHAVIOUR,
                value.message_type(),
            )),
        }
    }
}

impl From<UpdateStateProxyMessage> for ProxyMessage {
    fn from(value: UpdateStateProxyMessage) -> Self {
        ProxyMessage::UpdateState(value)
    }
}

impl From<VerifyMembershipProxyMessage> for ProxyMessage {
    fn from(value: VerifyMembershipProxyMessage) -> Self {
        ProxyMessage::VerifyMembership(value)
    }
}

impl From<MisbehaviourProxyMessage> for ProxyMessage {
    fn from(value: MisbehaviourProxyMessage) -> Self {
        ProxyMessage::Misbehaviour(value)
    }
}

sol! {
    struct EthABIHeaderedMessage {
        bytes32 header;
        bytes message;
    }
}

impl EthABIEncoder for ProxyMessage {
    fn ethabi_encode(self) -> Vec<u8> {
        EthABIHeaderedMessage {
            header: self.header().into(),
            message: match self {
                Self::UpdateState(c) => c.ethabi_encode(),
                Self::VerifyMembership(c) => c.ethabi_encode(),
                Self::Misbehaviour(c) => c.ethabi_encode(),
            },
        }
        .abi_encode()
    }

    fn ethabi_decode(bz: &[u8]) -> Result<Self, Error> {
        let eth_abi_message = EthABIHeaderedMessage::abi_decode(bz, true)?;
        let (version, message_type) = {
            let header = &eth_abi_message.header;
            if header.len() != MESSAGE_HEADER_SIZE {
                return Err(Error::invalid_message_header(format!(
                    "invalid header length: expected={MESSAGE_HEADER_SIZE} actual={} header={:?}",
                    header.len(),
                    eth_abi_message.header
                )));
            }
            let mut version = [0u8; 2];
            version.copy_from_slice(&header[0..=1]);
            let mut commitment_type = [0u8; 2];
            commitment_type.copy_from_slice(&header[2..=3]);
            (
                u16::from_be_bytes(version),
                u16::from_be_bytes(commitment_type),
            )
        };
        if version != MESSAGE_SCHEMA_VERSION {
            return Err(Error::invalid_message_header(format!(
                "invalid version: expected={} actual={} header={:?}",
                MESSAGE_SCHEMA_VERSION, version, eth_abi_message.header
            )));
        }
        let message = eth_abi_message.message;
        match message_type {
            MESSAGE_TYPE_UPDATE_STATE => {
                Ok(UpdateStateProxyMessage::ethabi_decode(&message)?.into())
            }
            MESSAGE_TYPE_STATE => Ok(VerifyMembershipProxyMessage::ethabi_decode(&message)?.into()),
            MESSAGE_TYPE_MISBEHAVIOUR => {
                Ok(MisbehaviourProxyMessage::ethabi_decode(&message)?.into())
            }
            _ => Err(Error::invalid_abi(format!(
                "invalid message type: {}",
                message_type
            ))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        message::verify_membership::CommitmentPrefix, CommitmentProof, StateID,
        TrustingPeriodContext,
    };
    use crypto::Address;
    use lcp_types::{nanos_to_duration, Any, Height, Time, MAX_UNIX_TIMESTAMP_NANOS};
    use proptest::prelude::*;

    fn height_from_tuple(tuple: (u64, u64)) -> Height {
        Height::new(tuple.0, tuple.1)
    }

    fn test_update_client_message(
        c1: UpdateStateProxyMessage,
        proof_signer: Address,
        proof_signature: Vec<u8>,
    ) {
        let v = c1.clone().ethabi_encode();
        let c2 = UpdateStateProxyMessage::ethabi_decode(&v).unwrap();
        assert_eq!(c1, c2);

        let p1 = CommitmentProof {
            message: ProxyMessage::from(c1).to_bytes(),
            signer: proof_signer,
            signature: proof_signature.to_vec(),
        };
        // TODO uncomment this line when we want to generate the test data
        // println!("{{\"{}\"}},", hex::encode(p1.clone().ethabi_encode()));
        let p2 = CommitmentProof::ethabi_decode(&p1.clone().ethabi_encode()).unwrap();
        assert_eq!(p1, p2);
    }

    proptest! {
        #[test]
        fn pt_update_client_message_with_empty_context(
            prev_height in any::<Option<(u64, u64)>>().prop_map(|v| v.map(height_from_tuple)),
            prev_state_id in any::<Option<[u8; 32]>>().prop_map(|v| v.map(StateID::from)),
            post_height in any::<(u64, u64)>().prop_map(height_from_tuple),
            post_state_id in any::<[u8; 32]>().prop_map(StateID::from),
            emitted_states in any::<Vec<((u64, u64), (String, Vec<u8>))>>(),
            timestamp in ..=MAX_UNIX_TIMESTAMP_NANOS,
            proof_signer in any::<[u8; 20]>(),
            proof_signature in any::<[u8; 65]>()
        ) {
            let c1 = UpdateStateProxyMessage {
                prev_height,
                prev_state_id,
                post_height,
                post_state_id,
                emitted_states: emitted_states.into_iter().map(|(height, (type_url, value))| {
                    EmittedState(height_from_tuple(height), Any::new(format!("/{}", type_url), value))
                }).collect(),
                timestamp: Time::from_unix_timestamp_nanos(timestamp).unwrap(),
                context: Default::default(),
            };
            test_update_client_message(c1, Address(proof_signer), proof_signature.to_vec());
        }

        #[test]
        fn pt_update_client_message_with_trusting_period_context(
            prev_height in any::<Option<(u64, u64)>>().prop_map(|v| v.map(height_from_tuple)),
            prev_state_id in any::<Option<[u8; 32]>>().prop_map(|v| v.map(StateID::from)),
            post_height in any::<(u64, u64)>().prop_map(height_from_tuple),
            post_state_id in any::<[u8; 32]>().prop_map(StateID::from),
            emitted_states in any::<Vec<((u64, u64), (String, Vec<u8>))>>(),
            timestamp in ..=MAX_UNIX_TIMESTAMP_NANOS,
            proof_signer in any::<[u8; 20]>(),
            proof_signature in any::<[u8; 65]>(),
            trusting_period in ..=MAX_UNIX_TIMESTAMP_NANOS,
            clock_drift in ..=MAX_UNIX_TIMESTAMP_NANOS,
            untrusted_header_timestamp in ..=MAX_UNIX_TIMESTAMP_NANOS,
            trusted_state_timestamp in ..=MAX_UNIX_TIMESTAMP_NANOS
        ) {
            let c1 = UpdateStateProxyMessage {
                prev_height,
                prev_state_id,
                post_height,
                post_state_id,
                emitted_states: emitted_states.into_iter().map(|(height, (type_url, value))| {
                    EmittedState(height_from_tuple(height), Any::new(format!("/{}", type_url), value))
                }).collect(),
                timestamp: Time::from_unix_timestamp_nanos(timestamp).unwrap(),
                context: TrustingPeriodContext::new(
                    nanos_to_duration(trusting_period).unwrap(),
                    nanos_to_duration(clock_drift).unwrap(),
                    Time::from_unix_timestamp_nanos(untrusted_header_timestamp).unwrap(),
                    Time::from_unix_timestamp_nanos(trusted_state_timestamp).unwrap(),
                ).into(),
            };
            test_update_client_message(c1, Address(proof_signer), proof_signature.to_vec());
        }

        #[test]
        fn pt_verify_membership(
            prefix in any::<CommitmentPrefix>(),
            path in any::<String>(),
            value in any::<Option<[u8; 32]>>(),
            height in any::<(u64, u64)>().prop_map(height_from_tuple),
            state_id in any::<[u8; 32]>().prop_map(StateID::from),
            proof_signer in any::<[u8; 20]>(),
            proof_signature in any::<[u8; 65]>()
        ) {
            let c1 = VerifyMembershipProxyMessage {
                prefix,
                path,
                value,
                height,
                state_id,
            };
            let v = c1.clone().ethabi_encode();
            let c2 = VerifyMembershipProxyMessage::ethabi_decode(&v).unwrap();
            assert_eq!(c1, c2);

            let p1 = CommitmentProof {
                message: ProxyMessage::from(c1).to_bytes(),
                signer: Address(proof_signer),
                signature: proof_signature.to_vec(),
            };
            let p2 = CommitmentProof::ethabi_decode(&p1.clone().ethabi_encode()).unwrap();
            assert_eq!(p1, p2);
        }
    }
}

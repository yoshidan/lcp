use super::bytes_to_bytes32;
use crate::context::ValidationContext;
use crate::encoder::{EthABIEmittedState, EthABIEncoder, EthABIHeight};
use crate::prelude::*;
use crate::{Error, StateID};
use core::fmt::Display;
use lcp_types::{Any, Height, Time};
use prost::Message;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UpdateClientMessage {
    pub prev_height: Option<Height>,
    pub prev_state_id: Option<StateID>,
    pub post_height: Height,
    pub post_state_id: StateID,
    pub timestamp: Time,
    pub context: ValidationContext,
    pub emitted_states: Vec<EmittedState>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EmittedState(pub Height, pub Any);

impl Display for EmittedState {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "EmittedState(height: {}, state: {})",
            self.0,
            hex::encode(self.1.encode_to_vec())
        )
    }
}

impl UpdateClientMessage {
    pub fn aggregate(self, other: Self) -> Result<Self, Error> {
        if self.post_state_id != other.prev_state_id.unwrap_or_default() {
            return Err(Error::message_aggregation_failed(format!(
                "invalid prev_state_id: expected={} actual={}",
                self.post_state_id,
                other.prev_state_id.unwrap_or_default()
            )));
        }
        if self.post_height != other.prev_height.unwrap_or_default() {
            return Err(Error::message_aggregation_failed(format!(
                "invalid prev_height: expected={} actual={}",
                self.post_height,
                other.prev_height.unwrap_or_default()
            )));
        }
        Ok(Self {
            prev_height: self.prev_height,
            prev_state_id: self.prev_state_id,
            post_height: other.post_height,
            post_state_id: other.post_state_id,
            timestamp: other.timestamp,
            context: self.context.aggregate(other.context)?,
            emitted_states: [self.emitted_states, other.emitted_states].concat(),
        })
    }
}

impl Display for UpdateClientMessage {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "UpdateClient(prev_height: {}, prev_state_id: {}, post_height: {}, post_state_id: {}, timestamp: {}, context: {}, emitted_states: [{}])",
            self.prev_height.as_ref().map_or("None".to_string(), |h| h.to_string()),
            self.prev_state_id.as_ref().map_or("None".to_string(), |id| id.to_string()),
            self.post_height,
            self.post_state_id,
            self.timestamp.as_unix_timestamp_nanos(),
            self.context,
            self.emitted_states.iter().map(|v| v.to_string()).collect::<Vec<_>>().join(", ")
        )
    }
}

/// Aggregate a list of messages into a single message
pub fn aggregate_messages(
    messages: Vec<UpdateClientMessage>,
) -> Result<UpdateClientMessage, Error> {
    if messages.is_empty() {
        return Err(Error::message_aggregation_failed(
            "cannot aggregate empty messages".to_string(),
        ));
    }
    let mut messages = messages.into_iter();
    let mut message = messages.next().unwrap();
    for m in messages {
        message = message.aggregate(m)?;
    }
    Ok(message)
}

/// the struct is encoded as a tuple of 7 elements
pub(crate) struct EthABIUpdateClientMessage {
    pub prev_height: EthABIHeight,               // (u64, u64)
    pub prev_state_id: ethabi::FixedBytes,       // bytes32
    pub post_height: EthABIHeight,               // (u64, u64)
    pub post_state_id: ethabi::FixedBytes,       // bytes32
    pub timestamp: ethabi::Uint,                 // u128
    pub context: ethabi::Bytes,                  // bytes
    pub emitted_states: Vec<EthABIEmittedState>, // [((u64, u64), bytes)]
}

impl EthABIUpdateClientMessage {
    pub fn encode(self) -> Vec<u8> {
        use ethabi::Token;
        ethabi::encode(&[Token::Tuple(vec![
            Token::Tuple(self.prev_height.into()),
            Token::FixedBytes(self.prev_state_id),
            Token::Tuple(self.post_height.into()),
            Token::FixedBytes(self.post_state_id),
            Token::Uint(self.timestamp),
            Token::Bytes(self.context),
            Token::Array(
                self.emitted_states
                    .into_iter()
                    .map(|v| Token::Tuple(v.into()))
                    .collect(),
            ),
        ])])
    }

    pub fn decode(bz: &[u8]) -> Result<Self, Error> {
        use ethabi::ParamType;
        let tuple = ethabi::decode(
            &[ParamType::Tuple(vec![
                ParamType::Tuple(vec![ParamType::Uint(64), ParamType::Uint(64)]),
                ParamType::FixedBytes(32),
                ParamType::Tuple(vec![ParamType::Uint(64), ParamType::Uint(64)]),
                ParamType::FixedBytes(32),
                ParamType::Uint(64),
                ParamType::Bytes,
                ParamType::Array(Box::new(ParamType::Tuple(vec![
                    ParamType::Tuple(vec![ParamType::Uint(64), ParamType::Uint(64)]),
                    ParamType::Bytes,
                ]))),
            ])],
            bz,
        )?
        .into_iter()
        .next()
        .unwrap()
        .into_tuple()
        .unwrap();

        // if the decoding is successful, the length of the tuple should be 7
        assert!(tuple.len() == 7);
        let mut values = tuple.into_iter();
        Ok(Self {
            prev_height: values.next().unwrap().into_tuple().unwrap().try_into()?,
            prev_state_id: values.next().unwrap().into_fixed_bytes().unwrap(),
            post_height: values.next().unwrap().into_tuple().unwrap().try_into()?,
            post_state_id: values.next().unwrap().into_fixed_bytes().unwrap(),
            timestamp: values.next().unwrap().into_uint().unwrap(),
            context: values.next().unwrap().into_bytes().unwrap(),
            emitted_states: values
                .next()
                .unwrap()
                .into_array()
                .unwrap()
                .into_iter()
                .map(|v| EthABIEmittedState::try_from(v.into_tuple().unwrap()))
                .collect::<Result<Vec<_>, _>>()?,
        })
    }
}

impl From<UpdateClientMessage> for EthABIUpdateClientMessage {
    fn from(value: UpdateClientMessage) -> Self {
        use ethabi::*;
        Self {
            prev_height: value.prev_height.into(),
            prev_state_id: FixedBytes::from(
                value.prev_state_id.unwrap_or_default().to_vec().as_slice(),
            ),
            post_height: value.post_height.into(),
            post_state_id: FixedBytes::from(value.post_state_id.to_vec().as_slice()),
            timestamp: Uint::from(value.timestamp.as_unix_timestamp_nanos()),
            context: value.context.ethabi_encode(),
            emitted_states: value
                .emitted_states
                .into_iter()
                .map(EthABIEmittedState::from)
                .collect(),
        }
    }
}

impl TryFrom<EthABIUpdateClientMessage> for UpdateClientMessage {
    type Error = Error;
    fn try_from(value: EthABIUpdateClientMessage) -> Result<Self, Self::Error> {
        Ok(Self {
            prev_height: value.prev_height.into(),
            prev_state_id: bytes_to_bytes32(value.prev_state_id)?.map(StateID::from),
            post_height: value.post_height.into(),
            post_state_id: value.post_state_id.as_slice().try_into()?,
            timestamp: Time::from_unix_timestamp_nanos(value.timestamp.as_u128())?,
            context: ValidationContext::ethabi_decode(value.context.as_slice())?,
            emitted_states: value
                .emitted_states
                .into_iter()
                .map(EmittedState::try_from)
                .collect::<Result<Vec<_>, _>>()?,
        })
    }
}

impl EthABIEncoder for UpdateClientMessage {
    fn ethabi_encode(self) -> Vec<u8> {
        Into::<EthABIUpdateClientMessage>::into(self).encode()
    }

    fn ethabi_decode(bz: &[u8]) -> Result<Self, Error> {
        EthABIUpdateClientMessage::decode(bz).and_then(|v| v.try_into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::TrustingPeriodContext;
    use core::time::Duration;

    #[test]
    fn test_update_client_message_aggregation() {
        {
            let msg0 = UpdateClientMessage {
                prev_height: Some(Height::new(1, 1)),
                prev_state_id: Some(StateID::from([1u8; 32])),
                post_height: Height::new(2, 2),
                post_state_id: StateID::from([2u8; 32]),
                timestamp: Time::from_unix_timestamp_nanos(1).unwrap(),
                context: ValidationContext::default(),
                emitted_states: vec![],
            };
            let msg1 = UpdateClientMessage {
                prev_height: Some(Height::new(2, 2)),
                prev_state_id: Some(StateID::from([2u8; 32])),
                post_height: Height::new(3, 3),
                post_state_id: StateID::from([3u8; 32]),
                timestamp: Time::from_unix_timestamp_nanos(2).unwrap(),
                context: ValidationContext::default(),
                emitted_states: vec![],
            };
            let expected = UpdateClientMessage {
                prev_height: Some(Height::new(1, 1)),
                prev_state_id: Some(StateID::from([1u8; 32])),
                post_height: Height::new(3, 3),
                post_state_id: StateID::from([3u8; 32]),
                timestamp: Time::from_unix_timestamp_nanos(2).unwrap(),
                context: ValidationContext::default(),
                emitted_states: vec![],
            };
            assert_eq!(aggregate_messages(vec![msg0, msg1]).unwrap(), expected);
        }
        {
            let msg0 = UpdateClientMessage {
                prev_height: Some(Height::new(1, 1)),
                prev_state_id: Some(StateID::from([1u8; 32])),
                post_height: Height::new(2, 2),
                post_state_id: StateID::from([2u8; 32]),
                timestamp: Time::from_unix_timestamp_nanos(1).unwrap(),
                context: ValidationContext::default(),
                emitted_states: vec![EmittedState(
                    Height::new(1, 1),
                    Any::new("/foo".to_string(), vec![1u8; 32]),
                )],
            };
            let msg1 = UpdateClientMessage {
                prev_height: Some(Height::new(2, 2)),
                prev_state_id: Some(StateID::from([2u8; 32])),
                post_height: Height::new(3, 3),
                post_state_id: StateID::from([3u8; 32]),
                timestamp: Time::from_unix_timestamp_nanos(2).unwrap(),
                context: ValidationContext::default(),
                emitted_states: vec![EmittedState(
                    Height::new(2, 2),
                    Any::new("/bar".to_string(), vec![2u8; 32]),
                )],
            };
            let expected = UpdateClientMessage {
                prev_height: Some(Height::new(1, 1)),
                prev_state_id: Some(StateID::from([1u8; 32])),
                post_height: Height::new(3, 3),
                post_state_id: StateID::from([3u8; 32]),
                timestamp: Time::from_unix_timestamp_nanos(2).unwrap(),
                context: ValidationContext::default(),
                emitted_states: vec![
                    EmittedState(
                        Height::new(1, 1),
                        Any::new("/foo".to_string(), vec![1u8; 32]),
                    ),
                    EmittedState(
                        Height::new(2, 2),
                        Any::new("/bar".to_string(), vec![2u8; 32]),
                    ),
                ],
            };
            assert_eq!(aggregate_messages(vec![msg0, msg1]).unwrap(), expected);
        }
        {
            // trusting period aggregation
            let msg0 = UpdateClientMessage {
                prev_height: Some(Height::new(1, 1)),
                prev_state_id: Some(StateID::from([1u8; 32])),
                post_height: Height::new(2, 2),
                post_state_id: StateID::from([2u8; 32]),
                timestamp: Time::from_unix_timestamp_nanos(1).unwrap(),
                context: TrustingPeriodContext::new(
                    Duration::from_secs(1),
                    Duration::from_secs(2),
                    Time::from_unix_timestamp_nanos(1).unwrap(),
                    Time::from_unix_timestamp_nanos(2).unwrap(),
                )
                .into(),
                emitted_states: vec![],
            };
            let msg1 = UpdateClientMessage {
                prev_height: Some(Height::new(2, 2)),
                prev_state_id: Some(StateID::from([2u8; 32])),
                post_height: Height::new(3, 3),
                post_state_id: StateID::from([3u8; 32]),
                timestamp: Time::from_unix_timestamp_nanos(2).unwrap(),
                context: TrustingPeriodContext::new(
                    Duration::from_secs(1),
                    Duration::from_secs(2),
                    Time::from_unix_timestamp_nanos(2).unwrap(),
                    Time::from_unix_timestamp_nanos(3).unwrap(),
                )
                .into(),
                emitted_states: vec![],
            };
            let expected = UpdateClientMessage {
                prev_height: Some(Height::new(1, 1)),
                prev_state_id: Some(StateID::from([1u8; 32])),
                post_height: Height::new(3, 3),
                post_state_id: StateID::from([3u8; 32]),
                timestamp: Time::from_unix_timestamp_nanos(2).unwrap(),
                context: TrustingPeriodContext::new(
                    Duration::from_secs(1),
                    Duration::from_secs(2),
                    Time::from_unix_timestamp_nanos(2).unwrap(),
                    Time::from_unix_timestamp_nanos(2).unwrap(),
                )
                .into(),
                emitted_states: vec![],
            };
            assert_eq!(aggregate_messages(vec![msg0, msg1]).unwrap(), expected);
        }
        {
            // invalid prev_state_id
            let msg0 = UpdateClientMessage {
                prev_height: Some(Height::new(1, 1)),
                prev_state_id: Some(StateID::from([1u8; 32])),
                post_height: Height::new(2, 2),
                post_state_id: StateID::from([2u8; 32]),
                timestamp: Time::from_unix_timestamp_nanos(1).unwrap(),
                context: ValidationContext::default(),
                emitted_states: vec![],
            };
            let msg1 = UpdateClientMessage {
                prev_height: Some(Height::new(2, 2)),
                prev_state_id: Some(StateID::from([3u8; 32])),
                post_height: Height::new(3, 3),
                post_state_id: StateID::from([3u8; 32]),
                timestamp: Time::from_unix_timestamp_nanos(2).unwrap(),
                context: ValidationContext::default(),
                emitted_states: vec![],
            };
            assert!(msg0.aggregate(msg1).is_err());
        }
        {
            // invalid prev_height
            let msg0 = UpdateClientMessage {
                prev_height: Some(Height::new(1, 1)),
                prev_state_id: Some(StateID::from([1u8; 32])),
                post_height: Height::new(2, 2),
                post_state_id: StateID::from([2u8; 32]),
                timestamp: Time::from_unix_timestamp_nanos(1).unwrap(),
                context: ValidationContext::default(),
                emitted_states: vec![],
            };
            let msg1 = UpdateClientMessage {
                prev_height: Some(Height::new(3, 3)),
                prev_state_id: Some(StateID::from([2u8; 32])),
                post_height: Height::new(3, 3),
                post_state_id: StateID::from([3u8; 32]),
                timestamp: Time::from_unix_timestamp_nanos(2).unwrap(),
                context: ValidationContext::default(),
                emitted_states: vec![],
            };
            assert!(msg0.aggregate(msg1).is_err());
        }
        {
            // empty messages
            assert!(aggregate_messages(vec![]).is_err());
        }
        {
            // single message
            let msg0 = UpdateClientMessage {
                prev_height: Some(Height::new(1, 1)),
                prev_state_id: Some(StateID::from([1u8; 32])),
                post_height: Height::new(2, 2),
                post_state_id: StateID::from([2u8; 32]),
                timestamp: Time::from_unix_timestamp_nanos(1).unwrap(),
                context: ValidationContext::default(),
                emitted_states: vec![],
            };
            assert_eq!(aggregate_messages(vec![msg0.clone()]).unwrap(), msg0);
        }
        {
            // three messages
            let msg0 = UpdateClientMessage {
                prev_height: Some(Height::new(1, 1)),
                prev_state_id: Some(StateID::from([1u8; 32])),
                post_height: Height::new(2, 2),
                post_state_id: StateID::from([2u8; 32]),
                timestamp: Time::from_unix_timestamp_nanos(1).unwrap(),
                context: ValidationContext::default(),
                emitted_states: vec![],
            };
            let msg1 = UpdateClientMessage {
                prev_height: Some(Height::new(2, 2)),
                prev_state_id: Some(StateID::from([2u8; 32])),
                post_height: Height::new(3, 3),
                post_state_id: StateID::from([3u8; 32]),
                timestamp: Time::from_unix_timestamp_nanos(2).unwrap(),
                context: ValidationContext::default(),
                emitted_states: vec![],
            };
            let msg2 = UpdateClientMessage {
                prev_height: Some(Height::new(3, 3)),
                prev_state_id: Some(StateID::from([3u8; 32])),
                post_height: Height::new(4, 4),
                post_state_id: StateID::from([4u8; 32]),
                timestamp: Time::from_unix_timestamp_nanos(3).unwrap(),
                context: ValidationContext::default(),
                emitted_states: vec![],
            };
            let expected = UpdateClientMessage {
                prev_height: Some(Height::new(1, 1)),
                prev_state_id: Some(StateID::from([1u8; 32])),
                post_height: Height::new(4, 4),
                post_state_id: StateID::from([4u8; 32]),
                timestamp: Time::from_unix_timestamp_nanos(3).unwrap(),
                context: ValidationContext::default(),
                emitted_states: vec![],
            };
            assert_eq!(
                aggregate_messages(vec![msg0, msg1, msg2]).unwrap(),
                expected
            );
        }
    }
}

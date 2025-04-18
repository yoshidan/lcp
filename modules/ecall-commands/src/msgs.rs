use crate::errors::InputValidationError as Error;
use crate::light_client::*;
use crate::prelude::*;
use core::str::FromStr;
use crypto::Address;
use lcp_types::proto::lcp::service::elc::v1::{
    MsgAggregateMessages, MsgAggregateMessagesResponse, MsgCreateClient, MsgCreateClientResponse,
    MsgUpdateClient, MsgUpdateClientResponse, MsgVerifyMembership, MsgVerifyMembershipResponse,
    MsgVerifyNonMembership, MsgVerifyNonMembershipResponse,
    QueryClientRequest as MsgQueryClientRequest, QueryClientResponse as MsgQueryClientResponse,
};
use lcp_types::ClientId;

impl TryFrom<MsgCreateClient> for InitClientInput {
    type Error = Error;
    fn try_from(msg: MsgCreateClient) -> Result<Self, Error> {
        let any_client_state = msg
            .client_state
            .ok_or_else(|| Error::invalid_argument("client_state must be non-nil".into()))?
            .into();
        let any_consensus_state = msg
            .consensus_state
            .ok_or_else(|| Error::invalid_argument("consensus_state must be non-nil".into()))?
            .into();
        Ok(Self {
            client_id: msg.client_id,
            any_client_state,
            any_consensus_state,
            signer: Address::try_from(msg.signer.as_slice())?,
        })
    }
}

impl TryFrom<MsgUpdateClient> for UpdateClientInput {
    type Error = Error;
    fn try_from(msg: MsgUpdateClient) -> Result<Self, Error> {
        let any_header = msg
            .header
            .ok_or_else(|| Error::invalid_argument("header must be non-nil".into()))?
            .into();
        let client_id = ClientId::from_str(&msg.client_id)?;
        Ok(Self {
            client_id,
            any_header,
            include_state: msg.include_state,
            signer: Address::try_from(msg.signer.as_slice())?,
        })
    }
}

impl TryFrom<MsgAggregateMessages> for AggregateMessagesInput {
    type Error = Error;
    fn try_from(msg: MsgAggregateMessages) -> Result<Self, Error> {
        let signer = Address::try_from(msg.signer.as_slice())?;
        Ok(Self {
            signer,
            messages: msg.messages,
            signatures: msg.signatures,
        })
    }
}

impl TryFrom<MsgVerifyMembership> for VerifyMembershipInput {
    type Error = Error;

    fn try_from(msg: MsgVerifyMembership) -> Result<Self, Self::Error> {
        let client_id = ClientId::from_str(&msg.client_id)?;
        let proof = CommitmentProofPair(
            msg.proof_height
                .ok_or_else(|| Error::invalid_argument("proof_height must be non-nil".into()))?
                .into(),
            msg.proof,
        );
        Ok(Self {
            client_id,
            prefix: msg.prefix,
            proof,
            path: msg.path,
            value: msg.value,
            signer: Address::try_from(msg.signer.as_slice())?,
        })
    }
}

impl TryFrom<MsgVerifyNonMembership> for VerifyNonMembershipInput {
    type Error = Error;

    fn try_from(msg: MsgVerifyNonMembership) -> Result<Self, Self::Error> {
        let client_id = ClientId::from_str(&msg.client_id)?;
        let proof = CommitmentProofPair(
            msg.proof_height
                .ok_or_else(|| Error::invalid_argument("proof_height must be non-nil".into()))?
                .into(),
            msg.proof,
        );
        Ok(Self {
            client_id,
            prefix: msg.prefix,
            proof,
            path: msg.path,
            signer: Address::try_from(msg.signer.as_slice())?,
        })
    }
}

impl TryFrom<MsgQueryClientRequest> for QueryClientInput {
    type Error = Error;
    fn try_from(query: MsgQueryClientRequest) -> Result<Self, Error> {
        let client_id = ClientId::from_str(&query.client_id)?;
        Ok(Self { client_id })
    }
}

impl From<InitClientResponse> for MsgCreateClientResponse {
    fn from(res: InitClientResponse) -> Self {
        Self {
            message: res.proof.message,
            signature: res.proof.signature,
        }
    }
}

impl From<UpdateClientResponse> for MsgUpdateClientResponse {
    fn from(res: UpdateClientResponse) -> Self {
        Self {
            message: res.0.message,
            signature: res.0.signature,
        }
    }
}

impl From<AggregateMessagesResponse> for MsgAggregateMessagesResponse {
    fn from(res: AggregateMessagesResponse) -> Self {
        Self {
            message: res.0.message,
            signature: res.0.signature,
        }
    }
}

impl From<VerifyMembershipResponse> for MsgVerifyMembershipResponse {
    fn from(res: VerifyMembershipResponse) -> Self {
        Self {
            message: res.0.message,
            signature: res.0.signature,
        }
    }
}

impl From<VerifyNonMembershipResponse> for MsgVerifyNonMembershipResponse {
    fn from(res: VerifyNonMembershipResponse) -> Self {
        Self {
            message: res.0.message,
            signature: res.0.signature,
        }
    }
}

impl From<QueryClientResponse> for MsgQueryClientResponse {
    fn from(res: QueryClientResponse) -> Self {
        Self {
            found: res.found,
            client_state: res.any_client_state.map(Into::into),
            consensus_state: res.any_consensus_state.map(Into::into),
        }
    }
}

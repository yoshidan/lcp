use crate::prelude::*;
use flex_error::*;
use light_client::LightClientSpecificError;

define_error! {
    #[derive(Debug, PartialEq, Eq)]
    Error {
        UnexpectedClientType {
            type_url: String
        }
        |e| {
            format_args!("unexpected client_type: type_url={}", e.type_url)
        },

        Ics02
        [TraceError<ibc::core::ics02_client::error::ClientError>]
        |_| { "ICS02 client error" },

        Ics03
        [TraceError<ibc::core::ics03_connection::error::ConnectionError>]
        |_| { "ICS03 connection error" },

        Ics04
        [TraceError<ibc::core::ics04_channel::error::ChannelError>]
        |_| { "ICS04 channel error" },

        Ics23
        [TraceError<ibc::core::ics23_commitment::error::CommitmentError>]
        |_| { "ICS23 commitment error" },

        Commitment
        [light_client::commitments::Error]
        |_| { "Commitment error" },

        InvalidTimestamp
        |_| { "Invalid timestamp" },

        Time
        [light_client::types::TimeError]
        |_| { "Time error" },
    }
}

impl LightClientSpecificError for Error {}

impl From<light_client::commitments::Error> for Error {
    fn from(err: light_client::commitments::Error) -> Self {
        Error::commitment(err)
    }
}

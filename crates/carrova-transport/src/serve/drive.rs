//! Accept receiver-opened streams until the fetch-done stream drains.

use std::sync::Arc;

use quinn::ConnectionError;
use tokio::sync::mpsc;
use tokio::task::JoinSet;

use super::quota::Quota;
use super::stream::stream;
use super::{ReadJob, StreamEnd, add};
use crate::connection::{AllowedChunks, ServeEnd};
use crate::error::{ProtocolViolation, Result, SetupFailure, TransportError};
use crate::runtime::{Stop, Stoppable};
use crate::wire::CloseCode;

pub(super) async fn drive(
    conn: quinn::Connection,
    allowed: AllowedChunks,
    quota: Quota,
    jobs: mpsc::Sender<ReadJob>,
    stop: Arc<Stop>,
) -> Result<ServeEnd> {
    let mut streams = JoinSet::new();
    let mut end = ServeEnd::default();
    let result = async {
        let mut done = loop {
            tokio::select! {
                accepted = conn.accept_bi() => match accepted {
                    Ok((send, recv)) => {
                        streams.spawn(stream(
                            send,
                            recv,
                            allowed.clone(),
                            quota.clone(),
                            jobs.clone(),
                            Arc::clone(&stop),
                        ));
                    }
                    Err(e) => return closed(&e),
                },
                () = stop.stopped() => {
                    return Err(SetupFailure::ServingStopped.into());
                }
                Some(joined) = streams.join_next() => match flatten(joined)? {
                    StreamEnd::Served(s) => add(&mut end, s),
                    StreamEnd::Done(send) => break send,
                },
            }
        };
        while let Some(joined) = streams.join_next().await {
            match flatten(joined)? {
                StreamEnd::Served(s) => add(&mut end, s),
                StreamEnd::Done(_) => {
                    return Err(ProtocolViolation::FetchEndedTwice.into());
                }
            }
        }
        let _ = done.finish();
        Ok(())
    }
    .await;
    match result {
        Ok(()) => Ok(end),
        Err(e) => {
            // Reset every stream before closing: quinn does not send a
            // close frame while stream data is pending.
            stop.stop();
            while streams.join_next().await.is_some() {}
            if e.is_violation() {
                CloseCode::Protocol.close(&conn);
            }
            Err(e)
        }
    }
}

/// The receiver closing the connection normally also ends serving.
fn closed(err: &ConnectionError) -> Result<()> {
    match err {
        ConnectionError::ApplicationClosed(c)
            if CloseCode::from_wire(c.error_code) == Some(CloseCode::Normal) =>
        {
            Ok(())
        }
        e => Err(TransportError::connection(e)),
    }
}

fn flatten(joined: Result<Result<StreamEnd>, tokio::task::JoinError>) -> Result<StreamEnd> {
    joined
        .map_err(|e| TransportError::internal("chunk stream task", e))
        .and_then(|r| r)
}

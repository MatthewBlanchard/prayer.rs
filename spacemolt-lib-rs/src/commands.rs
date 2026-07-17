//! Generated typed command facade.

use std::future::Future;
use std::pin::Pin;

use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_json::Value;

use crate::account::Account;
use crate::errors::{ClientError, SpacemoltError};
use crate::protocol::{MutationResult, QueryResult, StateDelta};

pub use crate::schema::*;

/// Future returned by generated command methods.
pub type CommandFuture<T> = Pin<Box<dyn Future<Output = Result<T, ClientError>> + Send>>;

/// Query result with generated `structuredContent` type.
#[derive(Debug, Clone, PartialEq)]
pub struct TypedQueryResult<T = Value> {
    pub result: Value,
    pub structured_content: Option<T>,
}

impl<T: Serialize> TypedQueryResult<T> {
    /// Consume the query envelope and return structured content when present.
    pub fn into_value(self) -> Result<Value, serde_json::Error> {
        self.structured_content
            .map(serde_json::to_value)
            .transpose()
            .map(|structured| structured.unwrap_or(self.result))
    }
}

impl<T: DeserializeOwned> TypedQueryResult<T> {
    /// Consume the query envelope and return its generated response value.
    ///
    /// Some transports omit `structuredContent`; in that case the typed value
    /// is decoded from the protocol result without exposing that fallback to
    /// downstream crates.
    pub fn into_typed(self) -> Result<T, serde_json::Error> {
        match self.structured_content {
            Some(value) => Ok(value),
            None => serde_json::from_value(self.result),
        }
    }
}

/// Mutation result with generated `delta.details` type.
#[derive(Debug, Clone, PartialEq)]
pub struct TypedMutationResult<T = Value> {
    pub command: String,
    pub tick: u64,
    pub delta: StateDelta,
    pub details: Option<T>,
    pub auto_docked: bool,
    pub auto_undocked: bool,
}

impl Account {
    /// Generated typed command facade grouped by OpenAPI tool.
    pub fn commands(&self) -> Commands {
        Commands::new(self.clone())
    }
}

fn query_command<T>(
    account: &Account,
    tool: &'static str,
    action: &'static str,
    payload: Option<Value>,
) -> CommandFuture<TypedQueryResult<T>>
where
    T: DeserializeOwned + Send + 'static,
{
    let pending = account.query(tool, action, payload);
    Box::pin(async move {
        let result = pending.await?;
        typed_query_result(result)
    })
}

fn mutate_command<T>(
    account: &Account,
    tool: &'static str,
    action: &'static str,
    payload: Option<Value>,
) -> CommandFuture<TypedMutationResult<T>>
where
    T: DeserializeOwned + Send + 'static,
{
    let pending = account.mutate(tool, action, payload);
    Box::pin(async move {
        let result = pending.await?;
        typed_mutation_result(result)
    })
}

fn typed_query_result<T>(result: QueryResult) -> Result<TypedQueryResult<T>, ClientError>
where
    T: DeserializeOwned,
{
    Ok(TypedQueryResult {
        result: result.result,
        structured_content: result
            .structured_content
            .map(serde_json::from_value)
            .transpose()
            .map_err(decode_error)?,
    })
}

fn typed_mutation_result<T>(result: MutationResult) -> Result<TypedMutationResult<T>, ClientError>
where
    T: DeserializeOwned,
{
    let details = result
        .delta
        .get("details")
        .cloned()
        .map(serde_json::from_value)
        .transpose()
        .map_err(decode_error)?;
    Ok(TypedMutationResult {
        command: result.command,
        tick: result.tick,
        delta: result.delta,
        details,
        auto_docked: result.auto_docked,
        auto_undocked: result.auto_undocked,
    })
}

fn payload_from_params<T>(params: T) -> Result<Option<Value>, ClientError>
where
    T: Serialize,
{
    serde_json::to_value(params)
        .map(Some)
        .map_err(|err| ClientError::Server(SpacemoltError::new("encode_error", err.to_string())))
}

fn optional_payload_from_params<T>(params: Option<T>) -> Result<Option<Value>, ClientError>
where
    T: Serialize,
{
    params
        .map(payload_from_params)
        .transpose()
        .map(Option::flatten)
}

fn ready_err<T>(err: ClientError) -> CommandFuture<T>
where
    T: Send + 'static,
{
    Box::pin(async move { Err(err) })
}

fn decode_error(err: serde_json::Error) -> ClientError {
    ClientError::Server(SpacemoltError::new("decode_error", err.to_string()))
}

include!(concat!(env!("OUT_DIR"), "/commands.gen.rs"));

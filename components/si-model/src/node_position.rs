use crate::{ModelError, SiStorable};
use serde::{Deserialize, Serialize};
use si_data::{NatsTxn, NatsTxnError, PgTxn};
use thiserror::Error;

const NODE_POSITION_BY_NODE_ID: &str = include_str!("./queries/node_position_by_node_id.sql");

#[derive(Error, Debug)]
pub enum NodePositionError {
    #[error("error in core model functions: {0}")]
    Model(#[from] ModelError),
    #[error("nats txn error: {0}")]
    NatsTxn(#[from] NatsTxnError),
    #[error("pg error: {0}")]
    Pg(#[from] si_data::PgError),
    #[error("pg pool error: {0}")]
    PgPool(#[from] si_data::PgPoolError),
    #[error("json serialization error: {0}")]
    SerdeJson(#[from] serde_json::Error),
}

pub type NodePositionResult<T> = Result<T, NodePositionError>;

#[derive(Deserialize, Serialize, Debug, PartialEq, Eq, Clone)]
#[serde(rename_all = "camelCase")]
pub struct NodePosition {
    pub id: String,
    pub node_id: String,
    pub context_id: String,
    pub x: String,
    pub y: String,
    pub si_storable: SiStorable,
}

impl NodePosition {
    pub async fn new(
        txn: &PgTxn<'_>,
        nats: &NatsTxn,
        node_id: impl AsRef<str>,
        context_id: impl AsRef<str>,
        x: impl AsRef<str>,
        y: impl AsRef<str>,
        workspace_id: impl AsRef<str>,
    ) -> NodePositionResult<Self> {
        let node_id = node_id.as_ref();
        let context_id = context_id.as_ref();
        let x = x.as_ref();
        let y = y.as_ref();
        let workspace_id = workspace_id.as_ref();

        let row = txn
            .query_one(
                "SELECT object FROM node_position_create_v1($1, $2, $3, $4, $5)",
                &[&node_id, &context_id, &x, &y, &workspace_id],
            )
            .await?;
        let json: serde_json::Value = row.try_get("object")?;
        nats.publish(&json).await?;
        let object: NodePosition = serde_json::from_value(json)?;

        Ok(object)
    }

    pub async fn create_or_update(
        txn: &PgTxn<'_>,
        nats: &NatsTxn,
        node_id: impl AsRef<str>,
        context_id: impl AsRef<str>,
        x: impl AsRef<str>,
        y: impl AsRef<str>,
        workspace_id: impl AsRef<str>,
    ) -> NodePositionResult<Self> {
        let node_id = node_id.as_ref();
        let context_id = context_id.as_ref();
        let x = x.as_ref();
        let y = y.as_ref();
        let workspace_id = workspace_id.as_ref();

        let row = txn
            .query_one(
                "SELECT object FROM node_position_create_or_update_v1($1, $2, $3, $4, $5)",
                &[&node_id, &context_id, &x, &y, &workspace_id],
            )
            .await?;
        let json: serde_json::Value = row.try_get("object")?;
        nats.publish(&json).await?;
        let object: NodePosition = serde_json::from_value(json)?;

        Ok(object)
    }

    pub async fn get_by_node_id(
        txn: &PgTxn<'_>,
        node_id: impl AsRef<str>,
    ) -> NodePositionResult<Vec<NodePosition>> {
        let node_id = node_id.as_ref();

        let rows = txn.query(NODE_POSITION_BY_NODE_ID, &[&node_id]).await?;

        let mut results: Vec<Self> = Vec::new();
        for row in rows.into_iter() {
            let json: serde_json::Value = row.try_get("object")?;
            let object: Self = serde_json::from_value(json)?;
            results.push(object);
        }

        Ok(results)
    }

    pub async fn save(&mut self, txn: &PgTxn<'_>, nats: &NatsTxn) -> NodePositionResult<()> {
        let json = serde_json::to_value(&self)?;
        let row = txn
            .query_one("SELECT object FROM node_position_save_v1($1)", &[&json])
            .await?;
        let updated_result: serde_json::Value = row.try_get("object")?;
        nats.publish(&updated_result).await?;
        let mut updated: Self = serde_json::from_value(updated_result)?;
        std::mem::swap(self, &mut updated);
        Ok(())
    }
}

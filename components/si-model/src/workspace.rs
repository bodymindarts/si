use crate::SimpleStorable;
use serde::{Deserialize, Serialize};
use si_data::{NatsTxn, NatsTxnError, PgTxn};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum WorkspaceError {
    #[error("nats txn error: {0}")]
    NatsTxn(#[from] NatsTxnError),
    #[error("pg error: {0}")]
    Pg(#[from] si_data::PgError),
    #[error("serde error: {0}")]
    SerdeJson(#[from] serde_json::Error),
}

pub type WorkspaceResult<T> = Result<T, WorkspaceError>;

#[derive(Deserialize, Serialize, Debug, PartialEq, Eq, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Workspace {
    pub id: String,
    pub name: String,
    pub si_storable: SimpleStorable,
}

impl Workspace {
    pub async fn new(
        txn: &PgTxn<'_>,
        nats: &NatsTxn,
        name: impl Into<String>,
        billing_account_id: impl Into<String>,
        organization_id: impl Into<String>,
    ) -> WorkspaceResult<Workspace> {
        let name = name.into();
        let billing_account_id = billing_account_id.into();
        let organization_id = organization_id.into();

        let row = txn
            .query_one(
                "SELECT object FROM workspace_create_v1($1, $2, $3)",
                &[&name, &billing_account_id, &organization_id],
            )
            .await?;
        let workspace_json: serde_json::Value = row.try_get("object")?;
        nats.publish(&workspace_json).await?;
        let workspace: Workspace = serde_json::from_value(workspace_json)?;

        Ok(workspace)
    }

    pub async fn save(&self, txn: &PgTxn<'_>, nats: &NatsTxn) -> WorkspaceResult<Workspace> {
        let json = serde_json::to_value(self)?;
        let row = txn
            .query_one("SELECT object FROM workspace_save_v1($1)", &[&json])
            .await?;
        let updated_result: serde_json::Value = row.try_get("object")?;
        nats.publish(&updated_result).await?;
        let updated = serde_json::from_value(updated_result)?;
        Ok(updated)
    }

    pub async fn get(txn: &PgTxn<'_>, workspace_id: impl AsRef<str>) -> WorkspaceResult<Workspace> {
        let id = workspace_id.as_ref();
        let row = txn
            .query_one("SELECT object FROM workspace_get_v1($1)", &[&id])
            .await?;
        let workspace_json: serde_json::Value = row.try_get("object")?;
        let workspace = serde_json::from_value(workspace_json)?;
        Ok(workspace)
    }
}

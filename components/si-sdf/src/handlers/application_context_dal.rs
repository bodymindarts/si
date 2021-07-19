use crate::handlers::{authorize, validate_tenancy, HandlerError};
use serde::{Deserialize, Serialize};
use si_data::{NatsConn, PgPool};
use si_model::{application, ApplicationContext, ChangeSet, EditSession, SiClaims};

#[derive(Deserialize, Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct GetApplicationContextRequest {
    pub application_id: String,
    pub workspace_id: String,
}

pub type GetApplicationContextReply = ApplicationContext;

pub async fn get_application_context(
    claim: SiClaims,
    request: GetApplicationContextRequest,
    pg: PgPool,
) -> Result<impl warp::Reply, warp::reject::Rejection> {
    let mut conn = pg.get().await.map_err(HandlerError::from)?;
    let txn = conn.transaction().await.map_err(HandlerError::from)?;

    authorize(
        &txn,
        &claim.user_id,
        "applicationContextDal",
        "getApplicationContext",
    )
    .await?;
    validate_tenancy(
        &txn,
        "workspaces",
        &request.workspace_id,
        &claim.billing_account_id,
    )
    .await?;
    validate_tenancy(
        &txn,
        "entities",
        &request.application_id,
        &claim.billing_account_id,
    )
    .await?;

    let context = application::context(&txn, &request.application_id, &request.workspace_id)
        .await
        .map_err(HandlerError::from)?;

    Ok(warp::reply::json(&context))
}

#[derive(Deserialize, Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct CreateChangeSetAndEditSessionRequest {
    pub change_set_name: String,
    pub workspace_id: String,
}

#[derive(Deserialize, Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct CreateChangeSetAndEditSessionReply {
    pub change_set: ChangeSet,
    pub edit_session: EditSession,
}

pub async fn create_change_set_and_edit_session(
    claim: SiClaims,
    request: CreateChangeSetAndEditSessionRequest,
    pg: PgPool,
    nats_conn: NatsConn,
) -> Result<impl warp::Reply, warp::reject::Rejection> {
    let mut conn = pg.get().await.map_err(HandlerError::from)?;
    let txn = conn.transaction().await.map_err(HandlerError::from)?;
    let nats = nats_conn.transaction();

    authorize(
        &txn,
        &claim.user_id,
        "applicationContextDal",
        "createChangeSetAndEditSession",
    )
    .await?;
    validate_tenancy(
        &txn,
        "workspaces",
        &request.workspace_id,
        &claim.billing_account_id,
    )
    .await?;

    let change_set = ChangeSet::new(
        &txn,
        &nats,
        Some(request.change_set_name),
        request.workspace_id.clone(),
    )
    .await
    .map_err(HandlerError::from)?;

    let edit_session = EditSession::new(
        &txn,
        &nats,
        None,
        change_set.id.clone(),
        request.workspace_id.clone(),
    )
    .await
    .map_err(HandlerError::from)?;

    txn.commit().await.map_err(HandlerError::from)?;
    nats.commit().await.map_err(HandlerError::from)?;

    let reply = CreateChangeSetAndEditSessionReply {
        change_set,
        edit_session,
    };

    Ok(warp::reply::json(&reply))
}

#[derive(Deserialize, Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct GetChangeSetAndEditSessionRequest {
    pub change_set_id: String,
    pub edit_session_id: String,
}

#[derive(Deserialize, Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct GetChangeSetAndEditSessionReply {
    pub change_set: ChangeSet,
    pub edit_session: EditSession,
}

pub async fn get_change_set_and_edit_session(
    claim: SiClaims,
    request: GetChangeSetAndEditSessionRequest,
    pg: PgPool,
) -> Result<impl warp::Reply, warp::reject::Rejection> {
    let mut conn = pg.get().await.map_err(HandlerError::from)?;
    let txn = conn.transaction().await.map_err(HandlerError::from)?;

    authorize(
        &txn,
        &claim.user_id,
        "applicationContextDal",
        "getChangeSetAndEditSession",
    )
    .await?;
    validate_tenancy(
        &txn,
        "change_sets",
        &request.change_set_id,
        &claim.billing_account_id,
    )
    .await?;
    validate_tenancy(
        &txn,
        "edit_sessions",
        &request.edit_session_id,
        &claim.billing_account_id,
    )
    .await?;

    let change_set = ChangeSet::get(&txn, &request.change_set_id)
        .await
        .map_err(HandlerError::from)?;
    let edit_session = EditSession::get(&txn, &request.edit_session_id)
        .await
        .map_err(HandlerError::from)?;

    txn.commit().await.map_err(HandlerError::from)?;

    let reply = GetChangeSetAndEditSessionReply {
        change_set,
        edit_session,
    };
    Ok(warp::reply::json(&reply))
}

#[derive(Deserialize, Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct GetChangeSetRequest {
    pub change_set_id: String,
}

#[derive(Deserialize, Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct GetChangeSetReply {
    pub change_set: ChangeSet,
}

pub async fn get_change_set(
    claim: SiClaims,
    request: GetChangeSetRequest,
    pg: PgPool,
) -> Result<impl warp::Reply, warp::reject::Rejection> {
    let mut conn = pg.get().await.map_err(HandlerError::from)?;
    let txn = conn.transaction().await.map_err(HandlerError::from)?;

    authorize(
        &txn,
        &claim.user_id,
        "applicationContextDal",
        "getChangeSet",
    )
    .await?;
    validate_tenancy(
        &txn,
        "change_sets",
        &request.change_set_id,
        &claim.billing_account_id,
    )
    .await?;

    let change_set = ChangeSet::get(&txn, &request.change_set_id)
        .await
        .map_err(HandlerError::from)?;

    txn.commit().await.map_err(HandlerError::from)?;

    let reply = GetChangeSetReply { change_set };
    Ok(warp::reply::json(&reply))
}

#[derive(Deserialize, Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct CreateEditSessionAndGetChangeSetRequest {
    pub change_set_id: String,
}

#[derive(Deserialize, Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct CreateEditSessionAndGetChangeSetReply {
    pub change_set: ChangeSet,
    pub edit_session: EditSession,
}

pub async fn create_edit_session_and_get_change_set(
    claim: SiClaims,
    request: CreateEditSessionAndGetChangeSetRequest,
    pg: PgPool,
    nats_conn: NatsConn,
) -> Result<impl warp::Reply, warp::reject::Rejection> {
    let mut conn = pg.get().await.map_err(HandlerError::from)?;
    let txn = conn.transaction().await.map_err(HandlerError::from)?;
    let nats = nats_conn.transaction();

    authorize(
        &txn,
        &claim.user_id,
        "applicationContextDal",
        "getChangeSet",
    )
    .await?;
    validate_tenancy(
        &txn,
        "change_sets",
        &request.change_set_id,
        &claim.billing_account_id,
    )
    .await?;

    let change_set = ChangeSet::get(&txn, &request.change_set_id)
        .await
        .map_err(HandlerError::from)?;

    let edit_session = EditSession::new(
        &txn,
        &nats,
        None,
        change_set.id.clone(),
        change_set.si_storable.workspace_id.clone(),
    )
    .await
    .map_err(HandlerError::from)?;

    txn.commit().await.map_err(HandlerError::from)?;
    nats.commit().await.map_err(HandlerError::from)?;

    let reply = CreateEditSessionAndGetChangeSetReply {
        change_set,
        edit_session,
    };
    Ok(warp::reply::json(&reply))
}

#[derive(Deserialize, Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct CreateEditSessionRequest {
    pub change_set_id: String,
    pub workspace_id: String,
}

#[derive(Deserialize, Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct CreateEditSessionReply {
    pub edit_session: EditSession,
}

pub async fn create_edit_session(
    claim: SiClaims,
    request: CreateEditSessionRequest,
    pg: PgPool,
    nats_conn: NatsConn,
) -> Result<impl warp::Reply, warp::reject::Rejection> {
    let mut conn = pg.get().await.map_err(HandlerError::from)?;
    let txn = conn.transaction().await.map_err(HandlerError::from)?;
    let nats = nats_conn.transaction();

    authorize(
        &txn,
        &claim.user_id,
        "applicationContextDal",
        "createEditSession",
    )
    .await?;
    validate_tenancy(
        &txn,
        "workspaces",
        &request.workspace_id,
        &claim.billing_account_id,
    )
    .await?;

    let edit_session = EditSession::new(
        &txn,
        &nats,
        None,
        request.change_set_id.clone(),
        request.workspace_id.clone(),
    )
    .await
    .map_err(HandlerError::from)?;

    txn.commit().await.map_err(HandlerError::from)?;
    nats.commit().await.map_err(HandlerError::from)?;

    let reply = CreateEditSessionReply { edit_session };

    Ok(warp::reply::json(&reply))
}

#[derive(Deserialize, Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct CancelEditSessionRequest {
    pub edit_session_id: String,
    pub workspace_id: String,
}

#[derive(Deserialize, Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct CancelEditSessionReply {
    pub edit_session: EditSession,
}

pub async fn cancel_edit_session(
    claim: SiClaims,
    request: CancelEditSessionRequest,
    pg: PgPool,
    nats_conn: NatsConn,
) -> Result<impl warp::Reply, warp::reject::Rejection> {
    let mut conn = pg.get().await.map_err(HandlerError::from)?;
    let txn = conn.transaction().await.map_err(HandlerError::from)?;
    let nats = nats_conn.transaction();

    authorize(
        &txn,
        &claim.user_id,
        "applicationContextDal",
        "cancelEditSession",
    )
    .await?;
    validate_tenancy(
        &txn,
        "workspaces",
        &request.workspace_id,
        &claim.billing_account_id,
    )
    .await?;

    let mut edit_session = EditSession::get(&txn, &request.edit_session_id)
        .await
        .map_err(HandlerError::from)?;
    edit_session
        .cancel(&txn)
        .await
        .map_err(HandlerError::from)?;

    txn.commit().await.map_err(HandlerError::from)?;
    nats.commit().await.map_err(HandlerError::from)?;

    let reply = CancelEditSessionReply { edit_session };

    Ok(warp::reply::json(&reply))
}

#[derive(Deserialize, Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct SaveEditSessionRequest {
    pub edit_session_id: String,
    pub workspace_id: String,
}

#[derive(Deserialize, Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct SaveEditSessionReply {
    pub edit_session: EditSession,
}

pub async fn save_edit_session(
    claim: SiClaims,
    request: SaveEditSessionRequest,
    pg: PgPool,
    nats_conn: NatsConn,
) -> Result<impl warp::Reply, warp::reject::Rejection> {
    let mut conn = pg.get().await.map_err(HandlerError::from)?;
    let txn = conn.transaction().await.map_err(HandlerError::from)?;
    let nats = nats_conn.transaction();

    authorize(
        &txn,
        &claim.user_id,
        "applicationContextDal",
        "saveEditSession",
    )
    .await?;
    validate_tenancy(
        &txn,
        "workspaces",
        &request.workspace_id,
        &claim.billing_account_id,
    )
    .await?;

    let mut edit_session = EditSession::get(&txn, &request.edit_session_id)
        .await
        .map_err(HandlerError::from)?;
    edit_session
        .save_session(&txn)
        .await
        .map_err(HandlerError::from)?;

    txn.commit().await.map_err(HandlerError::from)?;
    nats.commit().await.map_err(HandlerError::from)?;

    let reply = SaveEditSessionReply { edit_session };

    Ok(warp::reply::json(&reply))
}

#[derive(Deserialize, Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ApplyChangeSetRequest {
    pub change_set_id: String,
    pub workspace_id: String,
}

#[derive(Deserialize, Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ApplyChangeSetReply {
    pub change_set: ChangeSet,
}

pub async fn apply_change_set(
    claim: SiClaims,
    request: ApplyChangeSetRequest,
    pg: PgPool,
    nats_conn: NatsConn,
) -> Result<impl warp::Reply, warp::reject::Rejection> {
    let mut conn = pg.get().await.map_err(HandlerError::from)?;
    let txn = conn.transaction().await.map_err(HandlerError::from)?;
    let nats = nats_conn.transaction();

    authorize(
        &txn,
        &claim.user_id,
        "applicationContextDal",
        "applyChangeSet",
    )
    .await?;
    validate_tenancy(
        &txn,
        "workspaces",
        &request.workspace_id,
        &claim.billing_account_id,
    )
    .await?;

    let mut change_set = ChangeSet::get(&txn, &request.change_set_id)
        .await
        .map_err(HandlerError::from)?;
    change_set.apply(&txn).await.map_err(HandlerError::from)?;

    txn.commit().await.map_err(HandlerError::from)?;
    nats.commit().await.map_err(HandlerError::from)?;

    let reply = ApplyChangeSetReply { change_set };

    Ok(warp::reply::json(&reply))
}

use crate::{
    system, ChangeSet, ChangeSetError, Edge, EdgeError, EdgeKind, EditSession, EditSessionError,
    Entity, EntityError, LabelList, LabelListItem, Node, NodeError, Resource, SystemError,
    Veritech,
};
use serde::{Deserialize, Serialize};
use si_data::{NatsConn, NatsTxn, NatsTxnError, PgPool, PgTxn};
use thiserror::Error;

pub const APPLICATION_LIST: &str = include_str!("./queries/application_list.sql");

#[derive(Error, Debug)]
pub enum ApplicationError {
    #[error("edge error: {0}")]
    Edge(#[from] EdgeError),
    #[error("entity error: {0}")]
    Entity(#[from] EntityError),
    #[error("changeset error: {0}")]
    ChangeSet(#[from] ChangeSetError),
    #[error("edit session error: {0}")]
    EditSession(#[from] EditSessionError),
    #[error("nats txn: {0}")]
    NatsTxn(#[from] NatsTxnError),
    #[error("node error: {0}")]
    Node(#[from] NodeError),
    #[error("pg error: {0}")]
    Pg(#[from] si_data::PgError),
    #[error("pg pool error: {0}")]
    PgPool(#[from] si_data::PgPoolError),
    #[error("serde error: {0}")]
    SerdeJson(#[from] serde_json::Error),
    #[error("system error: {0}")]
    System(#[from] SystemError),
}

pub type ApplicationResult<T> = Result<T, ApplicationError>;

#[derive(Deserialize, Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ChangeSetCounts {
    open: i32,
    closed: i32,
}

#[derive(Deserialize, Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ServiceWithResources {
    service: Entity,
    resources: Vec<Resource>,
}

#[derive(Deserialize, Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ApplicationListEntry {
    pub application: Entity,
    pub systems: Vec<Entity>,
    pub services_with_resources: Vec<ServiceWithResources>,
    pub change_set_counts: ChangeSetCounts,
}

pub async fn create(
    pg: PgPool,
    nats_conn: NatsConn,
    nats: &NatsTxn,
    veritech: &Veritech,
    application_name: impl Into<String>,
    workspace_id: impl Into<String>,
) -> ApplicationResult<ApplicationListEntry> {
    let application_name = application_name.into();
    let workspace_id = workspace_id.into();

    let mut conn = pg.get().await?;
    let txn = conn.transaction().await?;
    let mut change_set = ChangeSet::new(&txn, &nats, None, workspace_id.clone()).await?;
    let mut edit_session = EditSession::new(
        &txn,
        &nats,
        None,
        change_set.id.clone(),
        workspace_id.clone(),
    )
    .await?;
    txn.commit().await?;

    let txn = conn.transaction().await?;
    let application_node = Node::new(
        &pg,
        &txn,
        &nats_conn,
        &nats,
        &veritech,
        Some(application_name),
        "application",
        &workspace_id,
        &change_set.id,
        &edit_session.id,
    )
    .await?;
    edit_session.save_session(&txn).await?;
    change_set.apply(&txn).await?;
    let application = Entity::for_edit_session(
        &txn,
        application_node.object_id,
        change_set.id,
        edit_session.id,
    )
    .await?;
    system::assign_entity_to_system_by_name(&txn, &nats, "production", &application).await?;

    txn.commit().await?;

    let reply: ApplicationListEntry = ApplicationListEntry {
        application,
        systems: vec![],
        services_with_resources: vec![],
        change_set_counts: ChangeSetCounts { open: 0, closed: 1 },
    };
    Ok(reply)
}

pub async fn list(
    txn: &PgTxn<'_>,
    workspace_id: impl AsRef<str>,
) -> ApplicationResult<Vec<ApplicationListEntry>> {
    let workspace_id = workspace_id.as_ref();
    let rows = txn.query(APPLICATION_LIST, &[&workspace_id]).await?;

    let mut list = Vec::new();
    for row in rows.into_iter() {
        let json: serde_json::Value = row.try_get("application")?;
        let application: Entity = serde_json::from_value(json)?;
        list.push(ApplicationListEntry {
            application,
            systems: vec![],
            services_with_resources: vec![],
            change_set_counts: ChangeSetCounts { open: 0, closed: 1 },
        });
    }
    Ok(list)
}

#[derive(Deserialize, Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ApplicationContext {
    pub application_name: String,
    pub systems_list: LabelList,
    pub open_change_sets_list: LabelList,
    pub revisions_list: LabelList,
}

pub async fn context(
    txn: &PgTxn<'_>,
    application_id: impl AsRef<str>,
    workspace_id: impl AsRef<str>,
) -> ApplicationResult<ApplicationContext> {
    let application_id = application_id.as_ref();
    let workspace_id = workspace_id.as_ref();

    let application = Entity::for_head(&txn, &application_id).await?;

    let systems_list = system::list_as_labels(&txn, &workspace_id).await?;

    let open_change_sets_list = ChangeSet::open_list_as_labels(&txn, &workspace_id).await?;

    let revisions_list = ChangeSet::revision_list_as_labels(&txn, &workspace_id).await?;

    Ok(ApplicationContext {
        application_name: application.name,
        systems_list,
        open_change_sets_list,
        revisions_list,
    })
}

#[derive(Deserialize, Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ApplicationEntities {
    pub entity_list: LabelList,
}

pub async fn all_entities(
    txn: &PgTxn<'_>,
    application_id: impl AsRef<str>,
    change_set_id: Option<&String>,
    edit_session_id: Option<&String>,
) -> ApplicationResult<ApplicationEntities> {
    let application_id = application_id.as_ref();
    let mut entity_list: LabelList = Vec::new();

    let root_entity = Entity::for_head_or_change_set_or_edit_session(
        &txn,
        &application_id,
        change_set_id,
        edit_session_id,
    )
    .await?;

    let successors =
        Edge::direct_successor_edges_by_object_id(&txn, &EdgeKind::Includes, &root_entity.id)
            .await?;

    for edge in successors.into_iter() {
        let entity = match Entity::for_head_or_change_set_or_edit_session(
            &txn,
            &edge.head_vertex.object_id,
            change_set_id,
            edit_session_id,
        )
        .await
        {
            Ok(entity) => entity,
            Err(_e) => continue,
        };
        entity_list.push(LabelListItem {
            label: entity.name,
            value: edge.head_vertex.object_id,
        });
    }
    Ok(ApplicationEntities { entity_list })
}

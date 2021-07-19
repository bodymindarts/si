use crate::handlers::{self, HandlerError};
use si_data::{EventLogFS, NatsConn, PgPool};
use si_model::{SiClaims, Veritech};
use sodiumoxide::crypto::secretbox;
use warp::{filters::BoxedFilter, Filter};

#[tracing::instrument]
pub fn api(
    pg: &PgPool,
    nats_conn: &NatsConn,
    veritech: &Veritech,
    _event_log_fs: &EventLogFS,
    secret_key: &secretbox::Key,
) -> BoxedFilter<(impl warp::Reply,)> {
    signup_dal(pg, nats_conn, veritech)
        .or(session_dal(pg, secret_key))
        .or(application_dal(pg, nats_conn, veritech))
        .or(application_context_dal(pg, nats_conn))
        .or(schematic_dal(pg, nats_conn, veritech))
        .or(attribute_dal(pg, nats_conn, veritech))
        .or(resource_dal(pg, nats_conn, veritech))
        .or(secret_dal(pg, nats_conn))
        .or(workflow_dal(pg, nats_conn, veritech))
        .or(updates(pg, nats_conn))
        .or(cli(pg, nats_conn, veritech))
        .recover(handlers::handle_rejection)
        .boxed()
}

// Resource DAL
pub fn resource_dal(
    pg: &PgPool,
    nats_conn: &NatsConn,
    veritech: &Veritech,
) -> BoxedFilter<(impl warp::Reply,)> {
    resource_dal_get_resource(pg.clone())
        .or(resource_dal_sync_resource(
            pg.clone(),
            nats_conn.clone(),
            veritech.clone(),
        ))
        .boxed()
}

pub fn resource_dal_get_resource(pg: PgPool) -> BoxedFilter<(impl warp::Reply,)> {
    warp::path!("resourceDal" / "getResource")
        .and(authenticated(pg.clone()))
        .and(warp::query::<handlers::resource_dal::GetResourceRequest>())
        .and(with_pg(pg))
        .and_then(handlers::resource_dal::get_resource)
        .boxed()
}

pub fn resource_dal_sync_resource(
    pg: PgPool,
    nats_conn: NatsConn,
    veritech: Veritech,
) -> BoxedFilter<(impl warp::Reply,)> {
    warp::path!("resourceDal" / "syncResource")
        .and(warp::post())
        .and(authenticated(pg.clone()))
        .and(warp::body::json::<
            handlers::resource_dal::SyncResourceRequest,
        >())
        .and(with_pg(pg))
        .and(with_nats_conn(nats_conn))
        .and(with_veritech(veritech))
        .and_then(handlers::resource_dal::sync_resource)
        .boxed()
}

// Workflow DAL
pub fn workflow_dal(
    pg: &PgPool,
    nats_conn: &NatsConn,
    veritech: &Veritech,
) -> BoxedFilter<(impl warp::Reply,)> {
    workflow_dal_run_action(pg.clone(), nats_conn.clone(), veritech.clone())
        .or(workflow_dal_list_action(pg.clone()))
        .boxed()
}

pub fn workflow_dal_run_action(
    pg: PgPool,
    nats_conn: NatsConn,
    veritech: Veritech,
) -> BoxedFilter<(impl warp::Reply,)> {
    warp::path!("workflowDal" / "runAction")
        .and(warp::post())
        .and(authenticated(pg.clone()))
        .and(warp::body::json::<handlers::workflow_dal::RunActionRequest>())
        .and(with_pg(pg))
        .and(with_nats_conn(nats_conn))
        .and(with_veritech(veritech))
        .and_then(handlers::workflow_dal::run_action)
        .boxed()
}

pub fn workflow_dal_list_action(pg: PgPool) -> BoxedFilter<(impl warp::Reply,)> {
    warp::path!("workflowDal" / "listAction")
        .and(warp::get())
        .and(authenticated(pg.clone()))
        .and(warp::query::<handlers::workflow_dal::ListActionRequest>())
        .and(with_pg(pg))
        .and_then(handlers::workflow_dal::list_action)
        .boxed()
}

// The Web Socket CLI API
pub fn cli(
    pg: &PgPool,
    nats_conn: &NatsConn,
    veritech: &Veritech,
) -> BoxedFilter<(impl warp::Reply,)> {
    warp::path!("cli")
        .and(warp::ws())
        .and(warp::query::<crate::update::WebsocketToken>())
        .and(with_pg(pg.clone()))
        .and(with_nats_conn(nats_conn.clone()))
        .and(with_veritech(veritech.clone()))
        .and_then(handlers::cli::cli)
        .boxed()
}

// The Web Socket Update API
pub fn updates(pg: &PgPool, nats_conn: &NatsConn) -> BoxedFilter<(impl warp::Reply,)> {
    warp::path!("updates")
        .and(warp::ws())
        .and(warp::query::<crate::update::WebsocketToken>())
        .and(with_pg(pg.clone()))
        .and(with_nats_conn(nats_conn.clone()))
        .and_then(handlers::updates::update)
        .boxed()
}

// Session DAL
pub fn session_dal(pg: &PgPool, secret_key: &secretbox::Key) -> BoxedFilter<(impl warp::Reply,)> {
    session_dal_login(pg.clone(), secret_key.clone())
        .or(session_dal_restore_authentication(pg.clone()))
        .or(session_dal_get_defaults(pg.clone()))
        .boxed()
}

pub fn session_dal_login(
    pg: PgPool,
    secret_key: secretbox::Key,
) -> BoxedFilter<(impl warp::Reply,)> {
    warp::path!("sessionDal" / "login")
        .and(warp::post())
        .and(with_pg(pg))
        .and(with_secret_key(secret_key))
        .and(warp::body::json::<handlers::session_dal::LoginRequest>())
        .and_then(handlers::session_dal::login)
        .boxed()
}

pub fn session_dal_restore_authentication(pg: PgPool) -> BoxedFilter<(impl warp::Reply,)> {
    warp::path!("sessionDal" / "restoreAuthentication")
        .and(warp::get())
        .and(authenticated(pg.clone()))
        .and(with_pg(pg))
        .and_then(handlers::session_dal::restore_authentication)
        .boxed()
}

pub fn session_dal_get_defaults(pg: PgPool) -> BoxedFilter<(impl warp::Reply,)> {
    warp::path!("sessionDal" / "getDefaults")
        .and(warp::get())
        .and(authenticated(pg.clone()))
        .and(with_pg(pg))
        .and_then(handlers::session_dal::get_defaults)
        .boxed()
}

// Attribute DAL
pub fn attribute_dal(
    pg: &PgPool,
    nats_conn: &NatsConn,
    veritech: &Veritech,
) -> BoxedFilter<(impl warp::Reply,)> {
    attribute_dal_get_entity(pg.clone())
        .or(attribute_dal_discover(
            pg.clone(),
            nats_conn.clone(),
            veritech.clone(),
        ))
        .or(attribute_dal_import_implementation(
            pg.clone(),
            nats_conn.clone(),
            veritech.clone(),
        ))
        .or(attribute_dal_import_concept(
            pg.clone(),
            nats_conn.clone(),
            veritech.clone(),
        ))
        .or(attribute_dal_get_discovery_list(pg.clone()))
        .or(attribute_dal_get_implementations_list(pg.clone()))
        .or(attribute_dal_get_entity_list(pg.clone()))
        .or(attribute_dal_get_connections(pg.clone()))
        .or(attribute_dal_delete_connection(
            pg.clone(),
            nats_conn.clone(),
        ))
        .or(attribute_dal_get_input_labels(pg.clone()))
        .or(attribute_dal_update_entity(
            pg.clone(),
            nats_conn.clone(),
            veritech.clone(),
        ))
        .or(attribute_dal_check_qualifications(
            pg.clone(),
            nats_conn.clone(),
            veritech.clone(),
        ))
        .boxed()
}

pub fn attribute_dal_get_entity(pg: PgPool) -> BoxedFilter<(impl warp::Reply,)> {
    warp::path!("attributeDal" / "getEntity")
        .and(warp::get())
        .and(authenticated(pg.clone()))
        .and(warp::query::<handlers::attribute_dal::GetEntityRequest>())
        .and(with_pg(pg))
        .and_then(handlers::attribute_dal::get_entity)
        .boxed()
}

pub fn attribute_dal_discover(
    pg: PgPool,
    nats_conn: NatsConn,
    veritech: Veritech,
) -> BoxedFilter<(impl warp::Reply,)> {
    warp::path!("attributeDal" / "discover")
        .and(warp::post())
        .and(authenticated(pg.clone()))
        .and(warp::body::json::<handlers::attribute_dal::DiscoverRequest>())
        .and(with_pg(pg))
        .and(with_nats_conn(nats_conn))
        .and(with_veritech(veritech))
        .and_then(handlers::attribute_dal::discover)
        .boxed()
}

pub fn attribute_dal_import_implementation(
    pg: PgPool,
    nats_conn: NatsConn,
    veritech: Veritech,
) -> BoxedFilter<(impl warp::Reply,)> {
    warp::path!("attributeDal" / "importImplementation")
        .and(warp::post())
        .and(authenticated(pg.clone()))
        .and(warp::body::json::<
            handlers::attribute_dal::ImportImplementationRequest,
        >())
        .and(with_pg(pg))
        .and(with_nats_conn(nats_conn))
        .and(with_veritech(veritech))
        .and_then(handlers::attribute_dal::import_implementation)
        .boxed()
}

pub fn attribute_dal_import_concept(
    pg: PgPool,
    nats_conn: NatsConn,
    veritech: Veritech,
) -> BoxedFilter<(impl warp::Reply,)> {
    warp::path!("attributeDal" / "importConcept")
        .and(warp::post())
        .and(authenticated(pg.clone()))
        .and(warp::body::json::<
            handlers::attribute_dal::ImportConceptRequest,
        >())
        .and(with_pg(pg))
        .and(with_nats_conn(nats_conn))
        .and(with_veritech(veritech))
        .and_then(handlers::attribute_dal::import_concept)
        .boxed()
}

pub fn attribute_dal_get_discovery_list(pg: PgPool) -> BoxedFilter<(impl warp::Reply,)> {
    warp::path!("attributeDal" / "getDiscoveryList")
        .and(warp::get())
        .and(authenticated(pg.clone()))
        .and(warp::query::<
            handlers::attribute_dal::GetDiscoveryListRequest,
        >())
        .and(with_pg(pg))
        .and_then(handlers::attribute_dal::get_discovery_list)
        .boxed()
}

pub fn attribute_dal_get_implementations_list(pg: PgPool) -> BoxedFilter<(impl warp::Reply,)> {
    warp::path!("attributeDal" / "getImplementationsList")
        .and(warp::post())
        .and(authenticated(pg.clone()))
        .and(warp::body::json::<
            handlers::attribute_dal::GetImplementationsListRequest,
        >())
        .and(with_pg(pg))
        .and_then(handlers::attribute_dal::get_implementations_list)
        .boxed()
}

pub fn attribute_dal_get_entity_list(pg: PgPool) -> BoxedFilter<(impl warp::Reply,)> {
    warp::path!("attributeDal" / "getEntityList")
        .and(warp::get())
        .and(authenticated(pg.clone()))
        .and(warp::query::<handlers::attribute_dal::GetEntityListRequest>())
        .and(with_pg(pg))
        .and_then(handlers::attribute_dal::get_entity_list)
        .boxed()
}

pub fn attribute_dal_get_connections(pg: PgPool) -> BoxedFilter<(impl warp::Reply,)> {
    warp::path!("attributeDal" / "getConnections")
        .and(warp::get())
        .and(authenticated(pg.clone()))
        .and(warp::query::<handlers::attribute_dal::GetConnectionsRequest>())
        .and(with_pg(pg))
        .and_then(handlers::attribute_dal::get_connections)
        .boxed()
}

pub fn attribute_dal_delete_connection(
    pg: PgPool,
    nats_conn: NatsConn,
) -> BoxedFilter<(impl warp::Reply,)> {
    warp::path!("attributeDal" / "deleteConnection")
        .and(warp::post())
        .and(authenticated(pg.clone()))
        .and(warp::body::json::<
            handlers::attribute_dal::DeleteConnectionRequest,
        >())
        .and(with_pg(pg))
        .and(with_nats_conn(nats_conn))
        .and_then(handlers::attribute_dal::delete_connection)
        .boxed()
}

pub fn attribute_dal_get_input_labels(pg: PgPool) -> BoxedFilter<(impl warp::Reply,)> {
    warp::path!("attributeDal" / "getInputLabels")
        .and(warp::get())
        .and(authenticated(pg.clone()))
        .and(warp::query::<handlers::attribute_dal::GetInputLabelsRequest>())
        .and(with_pg(pg))
        .and_then(handlers::attribute_dal::get_input_labels)
        .boxed()
}

pub fn attribute_dal_update_entity(
    pg: PgPool,
    nats_conn: NatsConn,
    veritech: Veritech,
) -> BoxedFilter<(impl warp::Reply,)> {
    warp::path!("attributeDal" / "updateEntity")
        .and(warp::post())
        .and(authenticated(pg.clone()))
        .and(warp::body::json::<
            handlers::attribute_dal::UpdateEntityRequest,
        >())
        .and(with_pg(pg))
        .and(with_nats_conn(nats_conn))
        .and(with_veritech(veritech))
        .and_then(handlers::attribute_dal::update_entity)
        .boxed()
}

pub fn attribute_dal_check_qualifications(
    pg: PgPool,
    nats_conn: NatsConn,
    veritech: Veritech,
) -> BoxedFilter<(impl warp::Reply,)> {
    warp::path!("attributeDal" / "checkQualifications")
        .and(warp::post())
        .and(authenticated(pg.clone()))
        .and(warp::body::json::<
            handlers::attribute_dal::CheckQualificationsRequest,
        >())
        .and(with_pg(pg))
        .and(with_nats_conn(nats_conn))
        .and(with_veritech(veritech))
        .and_then(handlers::attribute_dal::check_qualifications)
        .boxed()
}

// Schematic DAL
pub fn schematic_dal(
    pg: &PgPool,
    nats_conn: &NatsConn,
    veritech: &Veritech,
) -> BoxedFilter<(impl warp::Reply,)> {
    schematic_dal_get_application_system_schematic(pg.clone())
        .or(schematic_dal_connection_create(
            pg.clone(),
            nats_conn.clone(),
            veritech.clone(),
        ))
        .or(schematic_dal_node_create_for_application(
            pg.clone(),
            nats_conn.clone(),
            veritech.clone(),
        ))
        .or(schematic_dal_update_node_position(
            pg.clone(),
            nats_conn.clone(),
        ))
        .or(schematic_dal_delete_node(pg.clone(), nats_conn.clone()))
        .boxed()
}

pub fn schematic_dal_get_application_system_schematic(
    pg: PgPool,
) -> BoxedFilter<(impl warp::Reply,)> {
    warp::path!("schematicDal" / "getApplicationSystemSchematic")
        .and(warp::get())
        .and(authenticated(pg.clone()))
        .and(warp::query::<
            handlers::schematic_dal::GetApplicationSystemSchematicRequest,
        >())
        .and(with_pg(pg))
        .and_then(handlers::schematic_dal::get_application_system_schematic)
        .boxed()
}

pub fn schematic_dal_node_create_for_application(
    pg: PgPool,
    nats_conn: NatsConn,
    veritech: Veritech,
) -> BoxedFilter<(impl warp::Reply,)> {
    warp::path!("schematicDal" / "nodeCreateForApplication")
        .and(warp::post())
        .and(authenticated(pg.clone()))
        .and(warp::body::json::<
            handlers::schematic_dal::NodeCreateForApplicationRequest,
        >())
        .and(with_pg(pg))
        .and(with_nats_conn(nats_conn))
        .and(with_veritech(veritech))
        .and_then(handlers::schematic_dal::node_create_for_application)
        .boxed()
}

pub fn schematic_dal_connection_create(
    pg: PgPool,
    nats_conn: NatsConn,
    veritech: Veritech,
) -> BoxedFilter<(impl warp::Reply,)> {
    warp::path!("schematicDal" / "connectionCreate")
        .and(warp::post())
        .and(authenticated(pg.clone()))
        .and(warp::body::json::<
            handlers::schematic_dal::ConnectionCreateRequest,
        >())
        .and(with_pg(pg))
        .and(with_nats_conn(nats_conn))
        .and(with_veritech(veritech))
        .and_then(handlers::schematic_dal::connection_create)
        .boxed()
}

pub fn schematic_dal_update_node_position(
    pg: PgPool,
    nats_conn: NatsConn,
) -> BoxedFilter<(impl warp::Reply,)> {
    warp::path!("schematicDal" / "updateNodePosition")
        .and(warp::post())
        .and(authenticated(pg.clone()))
        .and(warp::body::json::<
            handlers::schematic_dal::UpdateNodePositionRequest,
        >())
        .and(with_pg(pg))
        .and(with_nats_conn(nats_conn))
        .and_then(handlers::schematic_dal::update_node_position)
        .boxed()
}

pub fn schematic_dal_delete_node(
    pg: PgPool,
    nats_conn: NatsConn,
) -> BoxedFilter<(impl warp::Reply,)> {
    warp::path!("schematicDal" / "deleteNode")
        .and(warp::post())
        .and(authenticated(pg.clone()))
        .and(warp::body::json::<handlers::schematic_dal::DeleteNodeRequest>())
        .and(with_pg(pg))
        .and(with_nats_conn(nats_conn))
        .and_then(handlers::schematic_dal::delete_node)
        .boxed()
}

// Application Context DAL
pub fn application_context_dal(
    pg: &PgPool,
    nats_conn: &NatsConn,
) -> BoxedFilter<(impl warp::Reply,)> {
    application_context_dal_get_application_context(pg.clone())
        .or(application_context_dal_get_change_set_and_edit_session(
            pg.clone(),
        ))
        .or(
            application_context_dal_create_edit_session_and_get_change_set(
                pg.clone(),
                nats_conn.clone(),
            ),
        )
        .or(application_context_dal_create_change_set_and_edit_session(
            pg.clone(),
            nats_conn.clone(),
        ))
        .or(application_context_dal_create_edit_session(
            pg.clone(),
            nats_conn.clone(),
        ))
        .or(application_context_dal_cancel_edit_session(
            pg.clone(),
            nats_conn.clone(),
        ))
        .or(application_context_dal_save_edit_session(
            pg.clone(),
            nats_conn.clone(),
        ))
        .or(application_context_dal_apply_change_set(
            pg.clone(),
            nats_conn.clone(),
        ))
        .or(application_context_dal_get_change_set(pg.clone()))
        .boxed()
}

pub fn application_context_dal_get_application_context(
    pg: PgPool,
) -> BoxedFilter<(impl warp::Reply,)> {
    warp::path!("applicationContextDal" / "getApplicationContext")
        .and(warp::get())
        .and(authenticated(pg.clone()))
        .and(warp::query::<
            handlers::application_context_dal::GetApplicationContextRequest,
        >())
        .and(with_pg(pg))
        .and_then(handlers::application_context_dal::get_application_context)
        .boxed()
}

pub fn application_context_dal_create_change_set_and_edit_session(
    pg: PgPool,
    nats_conn: NatsConn,
) -> BoxedFilter<(impl warp::Reply,)> {
    warp::path!("applicationContextDal" / "createChangeSetAndEditSession")
        .and(warp::post())
        .and(authenticated(pg.clone()))
        .and(warp::body::json::<
            handlers::application_context_dal::CreateChangeSetAndEditSessionRequest,
        >())
        .and(with_pg(pg))
        .and(with_nats_conn(nats_conn))
        .and_then(handlers::application_context_dal::create_change_set_and_edit_session)
        .boxed()
}

pub fn application_context_dal_cancel_edit_session(
    pg: PgPool,
    nats_conn: NatsConn,
) -> BoxedFilter<(impl warp::Reply,)> {
    warp::path!("applicationContextDal" / "cancelEditSession")
        .and(warp::post())
        .and(authenticated(pg.clone()))
        .and(warp::body::json::<
            handlers::application_context_dal::CancelEditSessionRequest,
        >())
        .and(with_pg(pg))
        .and(with_nats_conn(nats_conn))
        .and_then(handlers::application_context_dal::cancel_edit_session)
        .boxed()
}

pub fn application_context_dal_save_edit_session(
    pg: PgPool,
    nats_conn: NatsConn,
) -> BoxedFilter<(impl warp::Reply,)> {
    warp::path!("applicationContextDal" / "saveEditSession")
        .and(warp::post())
        .and(authenticated(pg.clone()))
        .and(warp::body::json::<
            handlers::application_context_dal::SaveEditSessionRequest,
        >())
        .and(with_pg(pg))
        .and(with_nats_conn(nats_conn))
        .and_then(handlers::application_context_dal::save_edit_session)
        .boxed()
}

pub fn application_context_dal_create_edit_session(
    pg: PgPool,
    nats_conn: NatsConn,
) -> BoxedFilter<(impl warp::Reply,)> {
    warp::path!("applicationContextDal" / "createEditSession")
        .and(warp::post())
        .and(authenticated(pg.clone()))
        .and(warp::body::json::<
            handlers::application_context_dal::CreateEditSessionRequest,
        >())
        .and(with_pg(pg))
        .and(with_nats_conn(nats_conn))
        .and_then(handlers::application_context_dal::create_edit_session)
        .boxed()
}

pub fn application_context_dal_create_edit_session_and_get_change_set(
    pg: PgPool,
    nats_conn: NatsConn,
) -> BoxedFilter<(impl warp::Reply,)> {
    warp::path!("applicationContextDal" / "createEditSessionAndGetChangeSet")
        .and(warp::post())
        .and(authenticated(pg.clone()))
        .and(warp::body::json::<
            handlers::application_context_dal::CreateEditSessionAndGetChangeSetRequest,
        >())
        .and(with_pg(pg))
        .and(with_nats_conn(nats_conn))
        .and_then(handlers::application_context_dal::create_edit_session_and_get_change_set)
        .boxed()
}

pub fn application_context_dal_get_change_set(pg: PgPool) -> BoxedFilter<(impl warp::Reply,)> {
    warp::path!("applicationContextDal" / "getChangeSet")
        .and(warp::get())
        .and(authenticated(pg.clone()))
        .and(warp::query::<
            handlers::application_context_dal::GetChangeSetRequest,
        >())
        .and(with_pg(pg))
        .and_then(handlers::application_context_dal::get_change_set)
        .boxed()
}

pub fn application_context_dal_get_change_set_and_edit_session(
    pg: PgPool,
) -> BoxedFilter<(impl warp::Reply,)> {
    warp::path!("applicationContextDal" / "getChangeSetAndEditSession")
        .and(warp::get())
        .and(authenticated(pg.clone()))
        .and(warp::query::<
            handlers::application_context_dal::GetChangeSetAndEditSessionRequest,
        >())
        .and(with_pg(pg))
        .and_then(handlers::application_context_dal::get_change_set_and_edit_session)
        .boxed()
}

pub fn application_context_dal_apply_change_set(
    pg: PgPool,
    nats_conn: NatsConn,
) -> BoxedFilter<(impl warp::Reply,)> {
    warp::path!("applicationContextDal" / "applyChangeSet")
        .and(warp::post())
        .and(authenticated(pg.clone()))
        .and(warp::body::json::<
            handlers::application_context_dal::ApplyChangeSetRequest,
        >())
        .and(with_pg(pg))
        .and(with_nats_conn(nats_conn))
        .and_then(handlers::application_context_dal::apply_change_set)
        .boxed()
}

// Application DAL
#[tracing::instrument]
pub fn application_dal(
    pg: &PgPool,
    nats_conn: &NatsConn,
    veritech: &Veritech,
) -> BoxedFilter<(impl warp::Reply,)> {
    application_dal_create_application(pg.clone(), nats_conn.clone(), veritech.clone())
        .or(application_dal_list_applications(pg.clone()))
        .or(application_dal_deploy_services(
            pg.clone(),
            nats_conn.clone(),
            veritech.clone(),
        ))
        .or(application_dal_activity_summary(pg.clone()))
        .or(application_dal_changes_summary(pg.clone()))
        .or(application_dal_resource_summary(pg.clone()))
        .boxed()
}

pub fn application_dal_list_applications(pg: PgPool) -> BoxedFilter<(impl warp::Reply,)> {
    warp::path!("applicationDal" / "listApplications")
        .and(warp::get())
        .and(authenticated(pg.clone()))
        .and(warp::query::<
            handlers::application_dal::ListApplicationsRequest,
        >())
        .and(with_pg(pg))
        .and_then(handlers::application_dal::list_applications)
        .boxed()
}

pub fn application_dal_activity_summary(pg: PgPool) -> BoxedFilter<(impl warp::Reply,)> {
    warp::path!("applicationDal" / "activitySummary")
        .and(warp::get())
        .and(authenticated(pg.clone()))
        .and(warp::query::<
            handlers::application_dal::ActivitySummaryRequest,
        >())
        .and(with_pg(pg))
        .and_then(handlers::application_dal::activity_summary)
        .boxed()
}

pub fn application_dal_changes_summary(pg: PgPool) -> BoxedFilter<(impl warp::Reply,)> {
    warp::path!("applicationDal" / "changesSummary")
        .and(warp::get())
        .and(authenticated(pg.clone()))
        .and(warp::query::<
            handlers::application_dal::ChangesSummaryRequest,
        >())
        .and(with_pg(pg))
        .and_then(handlers::application_dal::changes_summary)
        .boxed()
}

#[tracing::instrument(skip(pg))]
pub fn application_dal_resource_summary(pg: PgPool) -> BoxedFilter<(impl warp::Reply,)> {
    warp::path!("applicationDal" / "resourceSummary")
        .and(warp::get())
        .and(authenticated(pg.clone()))
        .and(warp::query::<
            handlers::application_dal::ResourceSummaryRequest,
        >())
        .and(with_pg(pg))
        .and_then(handlers::application_dal::resource_summary)
        .boxed()
}

pub fn application_dal_create_application(
    pg: PgPool,
    nats_conn: NatsConn,
    veritech: Veritech,
) -> BoxedFilter<(impl warp::Reply,)> {
    warp::path!("applicationDal" / "createApplication")
        .and(warp::post())
        .and(authenticated(pg.clone()))
        .and(warp::body::json::<
            handlers::application_dal::CreateApplicationRequest,
        >())
        .and(with_pg(pg))
        .and(with_nats_conn(nats_conn))
        .and(with_veritech(veritech))
        .and_then(handlers::application_dal::create_application)
        .boxed()
}

pub fn application_dal_deploy_services(
    pg: PgPool,
    nats_conn: NatsConn,
    veritech: Veritech,
) -> BoxedFilter<(impl warp::Reply,)> {
    warp::path!("applicationDal" / "deployServices")
        .and(warp::post())
        .and(authenticated(pg.clone()))
        .and(warp::body::json::<
            handlers::application_dal::DeployServicesRequest,
        >())
        .and(with_pg(pg))
        .and(with_nats_conn(nats_conn))
        .and(with_veritech(veritech))
        .and_then(handlers::application_dal::deploy_services)
        .boxed()
}

// Signup DAL
pub fn signup_dal(
    pg: &PgPool,
    nats_conn: &NatsConn,
    veritech: &Veritech,
) -> BoxedFilter<(impl warp::Reply,)> {
    signup_dal_create_billing_account(pg.clone(), nats_conn.clone(), veritech.clone()).boxed()
}

pub fn signup_dal_create_billing_account(
    pg: PgPool,
    nats_conn: NatsConn,
    veritech: Veritech,
) -> BoxedFilter<(impl warp::Reply,)> {
    warp::path!("signupDal" / "createBillingAccount")
        .and(warp::post())
        .and(with_pg(pg))
        .and(with_nats_conn(nats_conn))
        .and(with_veritech(veritech))
        .and(warp::body::json::<handlers::signup_dal::CreateRequest>())
        .and_then(handlers::signup_dal::create_billing_account)
        .boxed()
}

// Secret DAL
pub fn secret_dal(pg: &PgPool, nats_conn: &NatsConn) -> BoxedFilter<(impl warp::Reply,)> {
    secret_dal_get_public_key(pg.clone())
        .or(secret_dal_create_secret(pg.clone(), nats_conn.clone()))
        .or(secret_dal_list_secrets_for_workspace(pg.clone()))
        .boxed()
}

pub fn secret_dal_get_public_key(pg: PgPool) -> BoxedFilter<(impl warp::Reply,)> {
    warp::path!("secretDal" / "getPublicKey")
        .and(warp::get())
        .and(authenticated(pg.clone()))
        .and(with_pg(pg))
        .and_then(handlers::secret_dal::get_public_key)
        .boxed()
}

pub fn secret_dal_create_secret(
    pg: PgPool,
    nats_conn: NatsConn,
) -> BoxedFilter<(impl warp::Reply,)> {
    warp::path!("secretDal" / "createSecret")
        .and(warp::post())
        .and(authenticated(pg.clone()))
        .and(warp::body::json::<handlers::secret_dal::CreateSecretRequest>())
        .and(with_pg(pg))
        .and(with_nats_conn(nats_conn))
        .and_then(handlers::secret_dal::create_secret)
        .boxed()
}

pub fn secret_dal_list_secrets_for_workspace(pg: PgPool) -> BoxedFilter<(impl warp::Reply,)> {
    warp::path!("secretDal" / "listSecretsForWorkspace")
        .and(warp::get())
        .and(authenticated(pg.clone()))
        .and(warp::query::<
            handlers::secret_dal::ListSecretsForWorkspaceRequest,
        >())
        .and(with_pg(pg))
        .and_then(handlers::secret_dal::list_secrets_for_workspace)
        .boxed()
}

fn with_pg(
    pg: PgPool,
) -> impl Filter<Extract = (PgPool,), Error = std::convert::Infallible> + Clone {
    warp::any().map(move || pg.clone())
}

fn with_nats_conn(
    nats_conn: NatsConn,
) -> impl Filter<Extract = (NatsConn,), Error = std::convert::Infallible> + Clone {
    warp::any().map(move || nats_conn.clone())
}

#[allow(dead_code)]
fn with_event_log_fs(
    event_log_fs: EventLogFS,
) -> impl Filter<Extract = (EventLogFS,), Error = std::convert::Infallible> + Clone {
    warp::any().map(move || event_log_fs.clone())
}

fn with_veritech(
    veritech: Veritech,
) -> impl Filter<Extract = (Veritech,), Error = std::convert::Infallible> + Clone {
    warp::any().map(move || veritech.clone())
}

fn with_secret_key(
    secret_key: secretbox::Key,
) -> impl Filter<Extract = (secretbox::Key,), Error = std::convert::Infallible> + Clone {
    warp::any().map(move || secret_key.clone())
}

#[allow(dead_code)]
fn with_string(
    thingy: String,
) -> impl Filter<Extract = (String,), Error = std::convert::Infallible> + Clone {
    warp::any().map(move || thingy.clone())
}

#[derive(Debug, thiserror::Error)]
#[error("authorization header missing")]
struct MissingAuthorizationHeader;

impl warp::reject::Reject for MissingAuthorizationHeader {}

fn authenticated(
    pg: PgPool,
) -> impl Filter<Extract = (SiClaims,), Error = warp::reject::Rejection> + Clone {
    warp::header::optional("authorization")
        .and_then(|maybe: Option<String>| async move {
            match maybe {
                Some(hdr) => Ok(hdr),
                None => Err(warp::reject::custom(MissingAuthorizationHeader)),
            }
        })
        .and(warp::any().map(move || pg.clone()))
        .and_then(extract_claim)
}

#[tracing::instrument(skip(token, pg))]
async fn extract_claim(token: String, pg: PgPool) -> Result<SiClaims, warp::reject::Rejection> {
    let mut conn = pg.get().await.map_err(HandlerError::from)?;
    let txn = conn.transaction().await.map_err(HandlerError::from)?;
    let claims = si_model::user::authenticate(&txn, token)
        .await
        .map_err(HandlerError::from)?;
    Ok(claims)
}

//! Tests that run the store and application layers against a real Postgres.
//!
//! They need a migrated database (`make db-migrate`) and its URL in
//! `DATABASE_URL`, plus `AUTH_PUBLIC_BASE_URL` and `AUTH_ROOT_KEY_BASE64`;
//! `scripts/test_db.sh` sets all three. Without `DATABASE_URL` each test
//! skips, unless `BUWIZ_REQUIRE_DATABASE=1` (set in CI), where a missing
//! database fails instead of passing quietly.
//!
//! SQL goes through `store::test_pg`, which binds parameters the way Spin
//! does. Sessions and firm memberships, which wasi-auth resolves through
//! Spin, come from the fake registry here instead: a test makes a user,
//! gets its `RequestAuth`, and the application code sees a signed-in,
//! verified caller with the firm permissions the test gave it.
//!
//! Every test makes its own users, firms and TINs with random ids, so tests
//! share one database and run in parallel without seeing each other.

mod agent;
mod claims;
mod desktop_oauth;
mod invites;
mod records;

use std::collections::HashMap;
use std::sync::Mutex;

use serde_json::json;

use crate::application::RequestAuth;
use crate::contracts::{OrganizationSummary, SessionView};
use crate::error::{AuthStackError, AuthStackResult};
use crate::store::execute_sql;

/// `organization.update`: may manage the firm's passes and publishers.
pub(crate) const MANAGE: &str = "organization.update";

/// True when a database is configured. Without one the calling test should
/// return early, unless CI demands a database, which panics instead.
pub(crate) fn database() -> bool {
    if crate::store::env_non_empty("DATABASE_URL").is_some() {
        return true;
    }
    assert!(
        crate::store::env_non_empty("BUWIZ_REQUIRE_DATABASE").is_none(),
        "BUWIZ_REQUIRE_DATABASE is set but DATABASE_URL is not"
    );
    eprintln!("skipped: DATABASE_URL is not set");
    false
}

#[derive(Clone, Default)]
struct FakeSession {
    user_id: String,
    system_administrator: bool,
    /// System permissions, such as `system.user.manage` for claim review.
    permissions: Vec<String>,
    /// Firm id → the caller's permissions there.
    orgs: HashMap<String, Vec<String>>,
}

static SESSIONS: Mutex<Option<HashMap<String, FakeSession>>> = Mutex::new(None);

fn with_sessions<T>(f: impl FnOnce(&mut HashMap<String, FakeSession>) -> T) -> T {
    let mut guard = SESSIONS.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    f(guard.get_or_insert_with(HashMap::new))
}

/// The signed-in view for a fake session, or `None` when `auth` is not one
/// (the real lookup then runs).
pub(crate) fn session(auth: &RequestAuth) -> Option<SessionView> {
    let session_id = auth.session_id.as_deref()?;
    let fake = with_sessions(|sessions| sessions.get(session_id).cloned())?;
    Some(SessionView {
        authenticated: true,
        session_id: Some(session_id.to_owned()),
        public_session_id: None,
        tenant_id: None,
        user_id: Some(fake.user_id),
        primary_email: None,
        expires_at: None,
        permissions: fake.permissions,
        // Staff act after a step-up; members don't need one here.
        assurance: if fake.system_administrator { "aal2" } else { "aal1" }.to_owned(),
        system_administrator: fake.system_administrator,
        issued_at_unix_seconds: None,
        expires_at_unix_seconds: None,
    })
}

/// The caller's view of a firm for a fake session: its permissions when a
/// member, `Forbidden` when not. `None` when the session is not a fake one.
pub(crate) fn organization(
    session_id: &str,
    organization_id: &str,
) -> Option<AuthStackResult<OrganizationSummary>> {
    let fake = with_sessions(|sessions| sessions.get(session_id).cloned())?;
    Some(match fake.orgs.get(organization_id) {
        Some(permissions) => Ok(OrganizationSummary {
            organization_id: organization_id.to_owned(),
            name: "Test firm".to_owned(),
            slug: String::new(),
            status: "active".to_owned(),
            current_user_role: "member".to_owned(),
            permissions: permissions.clone(),
            created_at_ms: 0,
        }),
        None => Err(AuthStackError::Forbidden),
    })
}

/// Every firm a fake session's user belongs to, or `None` when `auth` is not
/// a fake session.
pub(crate) fn organizations(auth: &RequestAuth) -> Option<Vec<OrganizationSummary>> {
    let session_id = auth.session_id.as_deref()?;
    let fake = with_sessions(|sessions| sessions.get(session_id).cloned())?;
    let mut ids: Vec<&String> = fake.orgs.keys().collect();
    ids.sort();
    Some(
        ids.into_iter()
            .filter_map(|id| organization(session_id, id).and_then(Result::ok))
            .collect(),
    )
}

thread_local! {
    static ACTING: std::cell::RefCell<Option<RequestAuth>> = const { std::cell::RefCell::new(None) };
}

/// Server functions read the caller from the Leptos request, which a test
/// doesn't have: after this, they act as `user` on this test's thread.
pub(crate) fn act_as(user: &User) {
    ACTING.with(|acting| *acting.borrow_mut() = Some(user.auth()));
}

/// The caller set with [`act_as`] on this thread.
pub(crate) fn acting() -> Option<RequestAuth> {
    ACTING.with(|acting| acting.borrow().clone())
}

fn new_id() -> String {
    crate::store::new_uuid_v4().expect("uuid")
}

/// A verified user with a signed-in fake session.
#[derive(Clone, Debug)]
pub(crate) struct User {
    pub(crate) user_id: String,
    pub(crate) session_id: String,
}

impl User {
    pub(crate) fn auth(&self) -> RequestAuth {
        RequestAuth::from_parts(Some(self.session_id.clone()), None, None)
    }
}

pub(crate) async fn user() -> User {
    user_with_status("active").await
}

pub(crate) async fn user_with_status(status: &str) -> User {
    let user_id = new_id();
    let email = format!("{user_id}@test.invalid");
    execute_sql(
        "INSERT INTO auth_users (user_id, normalized_email, primary_email, status, created_at_ms, updated_at_ms) \
         VALUES (?1::uuid, ?2, ?2, ?3, 0::bigint, 0::bigint)",
        vec![json!(user_id), json!(email), json!(status)],
    )
    .await
    .expect("insert test user");
    let session_id = format!("test-session-{}", new_id());
    with_sessions(|sessions| {
        sessions.insert(
            session_id.clone(),
            FakeSession {
                user_id: user_id.clone(),
                ..FakeSession::default()
            },
        )
    });
    User { user_id, session_id }
}

/// Makes `user` Buwiz staff: a system administrator with
/// `system.user.manage` and a recent step-up (reviews ownership claims).
pub(crate) fn make_staff(user: &User) {
    with_sessions(|sessions| {
        if let Some(fake) = sessions.get_mut(&user.session_id) {
            fake.system_administrator = true;
            fake.permissions.push("system.user.manage".to_owned());
        }
    });
}

/// A new active firm created by `creator`, who joins it as its owner with
/// `permissions`. The firm, its roles and the owner go in one transaction:
/// wasi-auth checks at commit that an active firm has an owner.
pub(crate) async fn firm(creator: &User, permissions: &[&str]) -> String {
    use crate::store::AtomicSqlStatement as Sql;
    let organization_id = new_id();
    let org = json!(organization_id);
    let role = |role_id: &str, name: &str| {
        Sql::execute(
            "INSERT INTO auth_roles (organization_id, role_id, name, built_in, created_at_ms, updated_at_ms) \
             VALUES (?1::uuid, ?2, ?3, true, 0::bigint, 0::bigint)",
            vec![org.clone(), json!(role_id), json!(name)],
        )
    };
    crate::store::execute_sql_atomic(vec![
        Sql::execute(
            "INSERT INTO auth_organizations (organization_id, name, status, created_by, created_at_ms, updated_at_ms) \
             VALUES (?1::uuid, 'Test firm', 'active', ?2::uuid, 0::bigint, 0::bigint)",
            vec![org.clone(), json!(creator.user_id)],
        ),
        role("owner", "Owner"),
        role("member", "Member"),
        membership(&organization_id, creator, "owner"),
    ])
    .await
    .expect("insert test firm");
    remember(&organization_id, creator, permissions);
    organization_id
}

fn membership(organization_id: &str, user: &User, role_id: &str) -> crate::store::AtomicSqlStatement {
    crate::store::AtomicSqlStatement::execute(
        "INSERT INTO auth_memberships (organization_id, user_id, role_id, status, joined_at_ms, updated_at_ms) \
         VALUES (?1::uuid, ?2::uuid, ?3, 'active', 0::bigint, 0::bigint)",
        vec![json!(organization_id), json!(user.user_id), json!(role_id)],
    )
}

fn remember(organization_id: &str, user: &User, permissions: &[&str]) {
    let permissions = permissions.iter().map(|p| (*p).to_owned()).collect();
    with_sessions(|sessions| {
        if let Some(fake) = sessions.get_mut(&user.session_id) {
            fake.orgs.insert(organization_id.to_owned(), permissions);
        }
    });
}

/// Adds `user` to the firm as an active member with `permissions`.
pub(crate) async fn join(organization_id: &str, user: &User, permissions: &[&str]) {
    crate::store::execute_sql_atomic(vec![membership(organization_id, user, "member")])
        .await
        .expect("insert test membership");
    remember(organization_id, user, permissions);
}
/// A random 9-digit TIN no other test uses.
pub(crate) fn tin() -> String {
    let id = new_id().replace('-', "");
    let digits: String = id
        .bytes()
        .map(|b| char::from(b'0' + b % 10))
        .take(9)
        .collect();
    format!("9{}", &digits[1..])
}

/// `owner` connects a test agent app to `profile_id`, with no permissions.
/// Returns the agent as the MCP endpoint would see it.
pub(crate) async fn connected_agent(owner: &User, profile_id: &str) -> crate::store::AgentActor {
    let client_id = format!("test-client-{}", new_id());
    crate::store::insert_agent_client(&client_id, "Test agent", &["http://127.0.0.1:9/cb".to_owned()])
        .await
        .expect("client");
    let connection_id = new_id();
    crate::store::create_agent_connection(
        &connection_id,
        &owner.user_id,
        &client_id,
        crate::domain::PassPermissions::empty(),
        &[profile_id.to_owned()],
        &new_id(),
        "http://127.0.0.1:9/cb",
        "challenge",
        60,
        "http://127.0.0.1:9/mcp",
    )
    .await
    .expect("connection");
    crate::store::AgentActor {
        connection_id,
        user_id: owner.user_id.clone(),
        client_id,
        client_name: "Test agent".to_owned(),
        permissions: crate::domain::PassPermissions::empty(),
        profile_ids: vec![profile_id.to_owned()],
        resource: "http://127.0.0.1:9/mcp".to_owned(),
        expires_at: 0,
    }
}

/// A new tax profile created by `creator`: personal, or held by `firm` (a
/// draft no taxpayer owns yet). Returns its id and TIN root.
pub(crate) async fn profile(creator: &User, firm: Option<&str>) -> (String, String) {
    let tin_root = tin();
    let view = crate::application::create_tax_profile(
        crate::contracts::TaxProfileCreateRequest {
            tin_root: tin_root.clone(),
            branch_code: "00000".to_owned(),
            display_name: None,
            registered_name: "Test Taxpayer".to_owned(),
            rdo_code: "044".to_owned(),
            line_of_business: String::new(),
            registered_address: String::new(),
            zip_code: String::new(),
            phone: String::new(),
            email: String::new(),
            taxpayer_type: "individual".to_owned(),
            tax_classification: None,
            is_vat_registered: false,
            organization_id: firm.map(str::to_owned),
            org_id: None,
        },
        creator.auth(),
    )
    .await
    .expect("create profile");
    (view.profile_id, tin_root)
}

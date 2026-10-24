//! Device-code rows against Postgres (`store::desktop_oauth`).

use super::*;
use crate::store::{
    authorize_device_code, consume_device_code, insert_device_code, poll_device_code,
};

#[tokio::test(flavor = "current_thread")]
async fn a_device_code_authorizes_once() {
    if !database() {
        return;
    }
    let owner = user().await;
    let user_code = format!("TST{}", &owner.user_id[..8]).to_ascii_uppercase();
    let device_hash = format!("device-{}", owner.user_id);
    insert_device_code(
        &device_hash,
        &user_code,
        "buwiz-desktop",
        "http://localhost:3008/login/device",
        5,
        900,
    )
    .await
    .expect("insert device code");

    let pending = poll_device_code(&device_hash).await.expect("poll pending");
    assert!(!pending.authorized);
    assert!(pending.user_id.is_none());

    authorize_device_code(
        &user_code.to_ascii_lowercase(),
        &owner.user_id,
        &owner.session_id,
    )
    .await
    .expect("authorize");

    let approved = poll_device_code(&device_hash).await.expect("poll approved");
    assert!(approved.authorized);
    assert_eq!(approved.user_id.as_deref(), Some(owner.user_id.as_str()));

    let consumed = consume_device_code(&device_hash).await.expect("consume");
    assert!(consumed.authorized);
    assert_eq!(
        consumed.session_id.as_deref(),
        Some(owner.session_id.as_str())
    );

    let again = consume_device_code(&device_hash).await;
    assert!(again.is_err(), "{again:?}");
}

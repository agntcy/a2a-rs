// Copyright AGNTCY Contributors (https://github.com/agntcy)
// SPDX-License-Identifier: Apache-2.0

//! Integration tests that spawn a real subprocess via `StdioTransport`.
//!
//! These tests exercise paths that cannot be reached through the in-process
//! `tokio::io::duplex` setup used in `tests/e2e.rs`:
//!
//! - `HANDSHAKE_TIMEOUT` firing when the subprocess never sends a handshake.
//! - `CLOSE_TIMEOUT` + kill fallback when the subprocess ignores stdin EOF.
//! - The request-level lock that serializes concurrent client calls so
//!   responses cannot be paired with the wrong request.
//!
//! All tests use the `stdio_test_helper` binary defined in
//! `tests/bin/stdio_test_helper.rs`. Each test caps its own runtime via
//! `tokio::time::timeout` so a regression cannot hang CI.

use std::time::{Duration, Instant};

use a2a::*;
use a2a_client::transport::{ServiceParams, Transport};
use a2a_stdio::StdioTransport;

/// Path to the helper binary, resolved at compile time by Cargo.
const HELPER: &str = env!("CARGO_BIN_EXE_stdio_test_helper");

fn sample_send_message_request(task_id: &str) -> SendMessageRequest {
    SendMessageRequest {
        message: Message {
            message_id: format!("msg-{task_id}"),
            context_id: Some("ctx".to_string()),
            task_id: Some(task_id.to_string()),
            role: Role::User,
            parts: vec![Part::text("hello")],
            metadata: None,
            extensions: None,
            reference_task_ids: None,
        },
        configuration: None,
        metadata: None,
        tenant: None,
    }
}

#[tokio::test]
async fn spawn_fails_when_handshake_never_arrives() {
    // Helper hangs without writing the handshake. `spawn()` should return
    // a HandshakeFailed error after HANDSHAKE_TIMEOUT (10s) and not block
    // forever. We cap the test at 30s as a safety net.
    let res = tokio::time::timeout(
        Duration::from_secs(30),
        StdioTransport::spawn(HELPER, &["hang-no-handshake"], None),
    )
    .await
    .expect("spawn should return within HANDSHAKE_TIMEOUT, not hang");

    let err = match res {
        Err(e) => e,
        Ok(_) => panic!("spawn should fail when handshake never arrives"),
    };
    let msg = err.to_string();
    assert!(
        msg.to_lowercase().contains("handshake") || msg.to_lowercase().contains("timed out"),
        "unexpected error message: {msg}"
    );
}

#[tokio::test]
async fn destroy_kills_subprocess_that_ignores_stdin_close() {
    // Helper completes the handshake but then refuses to exit, even when
    // stdin closes. `destroy()` should hit CLOSE_TIMEOUT (5s), kill the
    // child, and return an error mentioning the timeout.
    let transport = StdioTransport::spawn(HELPER, &["handshake-then-hang"], None)
        .await
        .expect("handshake should succeed for handshake-then-hang");

    let started = Instant::now();
    let res = tokio::time::timeout(Duration::from_secs(20), transport.destroy())
        .await
        .expect("destroy should return within CLOSE_TIMEOUT, not hang");

    let err = res.expect_err("destroy should report a timeout when child ignores stdin close");
    assert!(
        err.to_string().to_lowercase().contains("did not exit"),
        "unexpected error message: {err}"
    );
    // Sanity: it should not have waited the full safety-net timeout.
    assert!(
        started.elapsed() < Duration::from_secs(15),
        "destroy took too long: {:?}",
        started.elapsed()
    );
}

#[tokio::test]
async fn concurrent_calls_are_serialized_and_responses_match_requests() {
    // The slow-echo helper replies after a fixed delay with a Task whose
    // `id` equals the request's `task_id`. We fire two concurrent calls
    // with distinct task_ids; each future must observe its own task_id.
    //
    // Without the request-level lock, the per-pipe writer/reader mutexes
    // alone would still let the two readers pick up each other's frames,
    // and the assertion below would fail.
    let transport = StdioTransport::spawn(HELPER, &["slow-echo"], None)
        .await
        .expect("handshake should succeed for slow-echo");

    let svc = ServiceParams::default();
    let req_a = sample_send_message_request("task-A");
    let req_b = sample_send_message_request("task-B");

    let (res_a, res_b) = tokio::join!(
        transport.send_message(&svc, &req_a),
        transport.send_message(&svc, &req_b),
    );

    let resp_a = res_a.expect("call A should succeed");
    let resp_b = res_b.expect("call B should succeed");

    match resp_a {
        SendMessageResponse::Task(t) => assert_eq!(t.id, "task-A", "A got wrong response"),
        other => panic!("expected Task for A, got {other:?}"),
    }
    match resp_b {
        SendMessageResponse::Task(t) => assert_eq!(t.id, "task-B", "B got wrong response"),
        other => panic!("expected Task for B, got {other:?}"),
    }

    // Clean shutdown — slow-echo exits cleanly when stdin closes.
    let _ = tokio::time::timeout(Duration::from_secs(10), transport.destroy()).await;
}

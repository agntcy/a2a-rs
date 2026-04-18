// Copyright AGNTCY Contributors (https://github.com/agntcy)
// SPDX-License-Identifier: Apache-2.0

//! Test-only subprocess helper for `agntcy-a2a-stdio` integration tests.
//!
//! This binary is spawned by tests in `tests/spawned.rs` to exercise paths
//! that require a real child process (handshake timeout, close timeout,
//! request serialization). It is not part of the public API.
//!
//! Modes (selected via the first CLI argument):
//!
//! - `hang-no-handshake` — block forever; never write the handshake.
//! - `handshake-then-hang` — write handshake, read ack, then block forever
//!   (ignores stdin EOF so `child.wait()` hangs).
//! - `slow-echo` — run a `StdioServer` whose `send_message` handler sleeps
//!   `STDIO_TEST_DELAY_MS` ms (default 200) before echoing the request's
//!   `task_id` back as a completed `Task`.

use std::env;
use std::sync::Arc;
use std::time::Duration;

use a2a::event::StreamResponse;
use a2a::*;
use a2a_server::RequestHandler;
use a2a_server::middleware::ServiceParams;
use a2a_stdio::handshake::{Handshake, read_handshake_ack};
use a2a_stdio::{StdioServer, handshake};
use async_trait::async_trait;
use futures::stream::{self, BoxStream};
use tokio::io::{BufReader, stdin, stdout};
use tokio::time::sleep;

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let mode = env::args().nth(1).unwrap_or_default();
    match mode.as_str() {
        "hang-no-handshake" => hang_no_handshake().await,
        "handshake-then-hang" => handshake_then_hang().await,
        "slow-echo" => slow_echo().await,
        other => {
            eprintln!("stdio_test_helper: unknown mode {other:?}");
            std::process::exit(2);
        }
    }
}

/// Block forever without writing anything. Drives `HANDSHAKE_TIMEOUT`.
async fn hang_no_handshake() {
    std::future::pending::<()>().await;
}

/// Complete the handshake, then block forever ignoring stdin EOF.
/// Drives `CLOSE_TIMEOUT` + kill fallback in the client.
async fn handshake_then_hang() {
    let mut writer = stdout();
    let mut reader = BufReader::new(stdin());

    let hs = Handshake::new(
        "test-session".to_string(),
        vec!["a2a/v1".to_string()],
    );
    handshake::write_handshake(&mut writer, &hs).await.unwrap();
    // Best-effort read of the ack; if it fails just hang anyway.
    let _ = read_handshake_ack(&mut reader).await;

    std::future::pending::<()>().await;
}

/// Run a `StdioServer` with a handler that sleeps before responding to
/// `message/send`. Used to verify concurrent client calls are serialized.
async fn slow_echo() {
    let delay_ms: u64 = env::var("STDIO_TEST_DELAY_MS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(200);
    let handler = Arc::new(SlowHandler {
        delay: Duration::from_millis(delay_ms),
    });
    let server = StdioServer::new(handler);
    let _ = server.run(stdin(), stdout()).await;
}

struct SlowHandler {
    delay: Duration,
}

#[async_trait]
impl RequestHandler for SlowHandler {
    async fn send_message(
        &self,
        _params: &ServiceParams,
        req: SendMessageRequest,
    ) -> Result<SendMessageResponse, A2AError> {
        sleep(self.delay).await;
        let task_id = req
            .message
            .task_id
            .clone()
            .unwrap_or_else(|| "unknown".to_string());
        Ok(SendMessageResponse::Task(Task {
            id: task_id,
            context_id: "ctx".to_string(),
            status: TaskStatus {
                state: TaskState::Completed,
                message: None,
                timestamp: None,
            },
            artifacts: None,
            history: None,
            metadata: None,
        }))
    }

    async fn send_streaming_message(
        &self,
        _params: &ServiceParams,
        _req: SendMessageRequest,
    ) -> Result<BoxStream<'static, Result<StreamResponse, A2AError>>, A2AError> {
        Ok(Box::pin(stream::empty()))
    }

    async fn get_task(
        &self,
        _params: &ServiceParams,
        req: GetTaskRequest,
    ) -> Result<Task, A2AError> {
        Err(A2AError::task_not_found(&req.id))
    }

    async fn list_tasks(
        &self,
        _params: &ServiceParams,
        _req: ListTasksRequest,
    ) -> Result<ListTasksResponse, A2AError> {
        Ok(ListTasksResponse {
            tasks: vec![],
            next_page_token: String::new(),
            page_size: 0,
            total_size: 0,
        })
    }

    async fn cancel_task(
        &self,
        _params: &ServiceParams,
        req: CancelTaskRequest,
    ) -> Result<Task, A2AError> {
        Err(A2AError::task_not_found(&req.id))
    }

    async fn subscribe_to_task(
        &self,
        _params: &ServiceParams,
        req: SubscribeToTaskRequest,
    ) -> Result<BoxStream<'static, Result<StreamResponse, A2AError>>, A2AError> {
        Err(A2AError::task_not_found(&req.id))
    }

    async fn create_push_config(
        &self,
        _params: &ServiceParams,
        req: CreateTaskPushNotificationConfigRequest,
    ) -> Result<TaskPushNotificationConfig, A2AError> {
        Ok(TaskPushNotificationConfig {
            task_id: req.task_id,
            tenant: req.tenant,
            config: req.config,
        })
    }

    async fn get_push_config(
        &self,
        _params: &ServiceParams,
        req: GetTaskPushNotificationConfigRequest,
    ) -> Result<TaskPushNotificationConfig, A2AError> {
        Err(A2AError::task_not_found(&req.task_id))
    }

    async fn list_push_configs(
        &self,
        _params: &ServiceParams,
        _req: ListTaskPushNotificationConfigsRequest,
    ) -> Result<ListTaskPushNotificationConfigsResponse, A2AError> {
        Ok(ListTaskPushNotificationConfigsResponse {
            configs: vec![],
            next_page_token: None,
        })
    }

    async fn delete_push_config(
        &self,
        _params: &ServiceParams,
        _req: DeleteTaskPushNotificationConfigRequest,
    ) -> Result<(), A2AError> {
        Ok(())
    }

    async fn get_extended_agent_card(
        &self,
        _params: &ServiceParams,
        _req: GetExtendedAgentCardRequest,
    ) -> Result<AgentCard, A2AError> {
        Err(A2AError::internal("not supported"))
    }
}

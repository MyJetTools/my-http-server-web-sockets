use std::{sync::Arc, time::Duration};

use my_http_server::HttpFailResult;

use super::MyWebSocket;

#[derive(Debug)]
pub enum WebSocketMessage {
    String(String),
    Binary(Vec<u8>),
}

#[async_trait::async_trait]
pub trait MyWebSocketCallback {
    async fn connected(
        &self,
        my_web_socket: Arc<MyWebSocket>,
        disconnect_timeout: Duration,
    ) -> Result<(), HttpFailResult>;
    async fn disconnected(&self, my_web_socket: Arc<MyWebSocket>);
    async fn on_message(&self, my_web_socket: Arc<MyWebSocket>, message: WebSocketMessage);
}

use std::sync::Arc;

use futures::{stream::SplitStream, StreamExt};
use hyper::upgrade::Upgraded;
use hyper_tungstenite::{
    tungstenite::{Error, Message},
    WebSocketStream,
};
use my_http_server::{HttpFailResult, HttpOkResult, HttpOutput, WebContentType};

use crate::{my_web_socket_callback::WebSocketMessage, MyWebSocket, MyWebSocketCallback};

pub async fn handle_web_socket_upgrade<
    TMyWebSocketCallback: MyWebSocketCallback + Send + Sync + 'static,
>(
    req: &mut hyper::Request<hyper::Body>,
    callback: Arc<TMyWebSocketCallback>,
    id: i64,
    addr: std::net::SocketAddr,
) -> Result<HttpOkResult, HttpFailResult> {
    let query_string = if let Some(query_string) = req.uri().query() {
        Some(query_string.to_string())
    } else {
        None
    };

    let upgrade_result = hyper_tungstenite::upgrade(req, None);

    if let Err(err) = upgrade_result {
        let content = format!("Can not upgrade websocket. Reason: {}", err);
        println!("{}", content);
        return Err(HttpFailResult {
            content_type: WebContentType::Text,
            status_code: 400,
            content: content.into_bytes(),
            write_telemetry: false,
            write_to_log: false,
        });
    }

    let (response, web_socket) = upgrade_result.unwrap();

    tokio::spawn(async move {
        let web_socket = web_socket.await.unwrap();

        let (write, read_stream) = web_socket.split();

        let my_web_socket = MyWebSocket::new(id, write, addr, query_string);
        let my_web_socket = Arc::new(my_web_socket);

        callback.connected(my_web_socket.clone()).await.unwrap();

        let serve_socket_result = tokio::spawn(serve_websocket(
            my_web_socket.clone(),
            read_stream,
            callback.clone(),
        ))
        .await;

        callback.disconnected(my_web_socket.clone()).await;

        if let Err(err) = serve_socket_result {
            println!(
                "Execution of websocket {} is finished with panic. {}",
                id, err
            );
        }
    });

    return Ok(HttpOkResult {
        write_telemetry: false,
        output: HttpOutput::Raw(response),
    });
}

/// Handle a websocket connection.
async fn serve_websocket<TMyWebSocketCallback: MyWebSocketCallback + Send + Sync + 'static>(
    my_web_socket: Arc<MyWebSocket>,
    mut read_stream: SplitStream<WebSocketStream<Upgraded>>,
    callback: Arc<TMyWebSocketCallback>,
) -> Result<(), Error> {
    while let Some(message) = read_stream.next().await {
        println!("WSMessage: {:?}", message);
        let result = match message? {
            Message::Text(msg) => {
                send_message(
                    my_web_socket.clone(),
                    WebSocketMessage::String(msg),
                    callback.clone(),
                )
                .await
            }
            Message::Binary(msg) => {
                send_message(
                    my_web_socket.clone(),
                    WebSocketMessage::Binary(msg),
                    callback.clone(),
                )
                .await
            }
            Message::Ping(_) => Ok(()),
            Message::Pong(_) => Ok(()),
            Message::Close(_) => Ok(()),
            Message::Frame(_) => Ok(()),
        };

        if let Err(err) = result {
            eprintln!("Error in websocket connection: {}", err);
            break;
        }
    }

    Ok(())
}

async fn send_message<TMyWebSocketCallback: MyWebSocketCallback + Send + Sync + 'static>(
    web_socket: Arc<MyWebSocket>,
    message: WebSocketMessage,
    callback: Arc<TMyWebSocketCallback>,
) -> Result<(), String> {
    let result = tokio::spawn(async move {
        callback.on_message(web_socket, message).await;
    })
    .await;

    if let Err(err) = result {
        return Err(format!("Error in on_message: {}", err));
    }

    Ok(())
}

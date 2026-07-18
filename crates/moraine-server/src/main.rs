//! Optional live review relay (Yjs WebSocket). In-memory only; no auth, no disk.
//! Wire format must stay compatible with src/lib/editor/yjsSession.ts.

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{Path, State};
use axum::response::IntoResponse;
use axum::routing::get;
use axum::Router;
use clap::Parser;
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use tokio::sync::{broadcast, RwLock};
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;
use tracing::{info, warn};

const DEFAULT_BIND: &str = "127.0.0.1:3099";
const BROADCAST_CAPACITY: usize = 256;

#[derive(Debug, Parser)]
#[command(
    name = "moraine-server",
    version,
    about = "Optional live review relay for Moraine (in-memory Yjs WebSocket)"
)]
struct Args {
    #[arg(long, env = "MORAINE_BIND", default_value = DEFAULT_BIND)]
    bind: String,
}

#[derive(Clone)]
struct AppState {
    rooms: Arc<RwLock<HashMap<String, Room>>>,
    next_peer: Arc<AtomicU64>,
}

struct Room {
    tx: broadcast::Sender<BusMsg>,
}

impl Room {
    fn new() -> Self {
        let (tx, _) = broadcast::channel(BROADCAST_CAPACITY);
        Self { tx }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct BusMsg {
    from: u64,
    payload: String,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    init_tracing();

    let state = AppState {
        rooms: Arc::new(RwLock::new(HashMap::new())),
        next_peer: Arc::new(AtomicU64::new(1)),
    };

    let app = Router::new()
        .route("/health", get(health))
        .route("/ws/{room_id}", get(ws_handler))
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    let addr: SocketAddr = args.bind.parse()?;
    info!("moraine-server on http://{addr}  GET /health  WS /ws/:room_id");

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app)
        .with_graceful_shutdown(async {
            let _ = tokio::signal::ctrl_c().await;
            info!("shutdown");
        })
        .await?;
    Ok(())
}

fn init_tracing() {
    let filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"));
    let _ = tracing_subscriber::fmt().with_env_filter(filter).try_init();
}

async fn health() -> impl IntoResponse {
    axum::Json(serde_json::json!({
        "ok": true,
        "service": "moraine-server",
        "version": env!("CARGO_PKG_VERSION"),
    }))
}

async fn ws_handler(
    ws: WebSocketUpgrade,
    Path(room_id): Path<String>,
    State(state): State<AppState>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| peer_loop(socket, room_id, state))
}

async fn peer_loop(socket: WebSocket, room_id: String, state: AppState) {
    let peer_id = state.next_peer.fetch_add(1, Ordering::Relaxed);
    info!(%room_id, peer_id, "connect");

    let tx = {
        let mut rooms = state.rooms.write().await;
        rooms
            .entry(room_id.clone())
            .or_insert_with(Room::new)
            .tx
            .clone()
    };
    let mut rx = tx.subscribe();
    let (mut sink, mut stream) = socket.split();

    let _ = tx.send(BusMsg {
        from: peer_id,
        payload: r#"{"type":"sync-request"}"#.to_string(),
    });

    loop {
        tokio::select! {
            incoming = stream.next() => {
                match incoming {
                    Some(Ok(Message::Text(text))) => {
                        if tx.send(BusMsg { from: peer_id, payload: text.to_string() }).is_err() {
                            break;
                        }
                    }
                    Some(Ok(Message::Binary(_))) => {}
                    Some(Ok(Message::Ping(p))) => {
                        if sink.send(Message::Pong(p)).await.is_err() {
                            break;
                        }
                    }
                    Some(Ok(Message::Pong(_))) | Some(Ok(Message::Close(_))) | None => break,
                    Some(Err(e)) => {
                        warn!(%room_id, peer_id, error = %e, "ws read");
                        break;
                    }
                }
            }
            outbound = rx.recv() => {
                match outbound {
                    Ok(msg) if msg.from != peer_id => {
                        if sink.send(Message::Text(msg.payload.into())).await.is_err() {
                            break;
                        }
                    }
                    Ok(_) => {}
                    Err(broadcast::error::RecvError::Lagged(n)) => {
                        warn!(%room_id, peer_id, lagged = n, "lag");
                    }
                    Err(broadcast::error::RecvError::Closed) => break,
                }
            }
        }
    }

    info!(%room_id, peer_id, "disconnect");
    let mut rooms = state.rooms.write().await;
    if let Some(room) = rooms.get(&room_id) {
        if room.tx.receiver_count() == 0 {
            rooms.remove(&room_id);
        }
    }
}

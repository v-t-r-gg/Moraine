# moraine-server

In-memory Yjs WebSocket relay. No auth, no disk persistence (v1).

```bash
cargo run -p moraine-server
# listens on 127.0.0.1:3099
# health: GET /health
# collab: WS  /ws/:room_id
```

Docker:

```bash
docker compose up --build
```

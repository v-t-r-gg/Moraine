# moraine-server

Optional in-memory Yjs WebSocket relay for **live** review sessions. No auth. No durable server-side room state.

```bash
cargo run -p moraine-server
# 127.0.0.1:3099
# GET /health
# WS  /ws/:room_id
```

Durable run records remain Markdown (+ sidecar) on disk, not in this process.

```bash
docker compose up --build
```

# Docker

Build and run the Nvisy server as a container.

## Standalone container

```sh
docker build -f docker/Dockerfile -t nvisy-server .
docker run -p 8080:8080 nvisy-server
```

The build context is the repository root. Run all commands from
there.

## Compose with externalised inference

`docker-compose.yml` pairs the runtime with the externalised
[`inference-gliner`](https://github.com/nvisycom/inference) Bento
for NER. Steps:

1. Copy `Nvisy.example.toml` to `Nvisy.toml` at the repo root, with
   `[detection.ner.backend]` set to:
   ```toml
   kind = "bento"
   base_url = "http://inference-gliner:3000"
   ```
2. From the repo root:
   ```sh
   docker compose -f docker/docker-compose.yml up --build
   ```
3. The runtime listens on `http://localhost:8080`.

## Configuration

Runtime configuration lives in `Nvisy.toml` (see
[`Nvisy.example.toml`](../Nvisy.example.toml)). The container
honours these env vars for bind address / port; everything else
comes from the TOML file or CLI flags forwarded by the entrypoint.

| Variable | Default | Description |
|----------|---------|-------------|
| `HOST` | `0.0.0.0` | Bind address |
| `PORT` | `8080` | HTTP listen port |

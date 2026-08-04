# Discord Keep-Alive

Keeps Discord accounts online, with optional status and presence.

## Run

With environment variables:

```bash
docker run --rm \
  -e TOKEN=your-token \
  -e DEVICE=desktop \
  -e STATUS=online \
  code.snipcola.st/snipcola/discord-keep-alive:latest
```

Or with a config file (see [`config.example.toml`](./config.example.toml)):

```bash
docker run --rm \
  -v "$PWD/config.toml:/config.toml:ro" \
  -e CONFIG_PATH=/config.toml \
  code.snipcola.st/snipcola/discord-keep-alive:latest
```

Compose example: [`docker-compose.example.yaml`](./docker-compose.example.yaml).

### From source

```bash
git clone --depth 1 --branch latest https://code.snipcola.st/snipcola/Discord-Keep-Alive
cd Discord-Keep-Alive
cp config.example.toml config.toml
# Set at least a token before running.
cargo run --release
```

## Configure

Full field reference: [`config.example.toml`](./config.example.toml).

- Precedence: **CLI > environment variables > config file > defaults**.
- Config path defaults to `./config.toml` (override with `--config` or `CONFIG_PATH`).
- Bots ignore user-only options (device, custom status, images, buttons, and similar) and send only one activity, due to gateway limits.

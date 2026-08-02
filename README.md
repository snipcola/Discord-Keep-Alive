# Discord Keep-Alive

Keeps Discord accounts online, with optional status and presence.

## Run

Deployment using environment variables for configuration:

```bash
docker run --rm \
  -e TOKEN=your-token \
  -e DEVICE=desktop \
  -e STATUS=online \
  code.snipcola.st/snipcola/discord-keep-alive:latest
```

Alternatively, using a config file (see [`config.example.toml`](./config.example.toml) for reference):

```bash
docker run --rm \
  -v "$PWD/config.toml:/config.toml:ro" \
  -e CONFIG_PATH=/config.toml \
  code.snipcola.st/snipcola/discord-keep-alive:latest
```

Compose example: [`docker-compose.example.yaml`](./docker-compose.example.yaml).

### From source

```bash
git clone https://code.snipcola.st/snipcola/Discord-Keep-Alive
cd Discord-Keep-Alive
cp config.example.toml config.toml
# Configure at least a token before continuing.
cargo run --release
```

## Configure

For in-depth configuration, see [`config.example.toml`](./config.example.toml).

- Configuration values take priority in this order: environment variables -> config file values -> hardcoded defaults.
- The config file path is, by default, `./config.toml` (override with `--config` or `CONFIG_PATH`).
- For bots, user-only options (device, custom status, images, buttons, and so on) are ignored, and only one activity is sent, due to gateway limitations.

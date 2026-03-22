# Reverse Proxy and TLS

Place nginx or Caddy in front of Life Engine Core to handle TLS termination with Let's Encrypt certificates. This guide covers both reverse proxies using the configuration templates shipped in the repository.

## Prerequisites

- Life Engine Core running on `127.0.0.1:3750` (Docker or bare-metal)
- A public domain name pointing to your server (required for Let's Encrypt)
- One of: nginx or Caddy installed on the host

## Why a reverse proxy

Core binds to `127.0.0.1:3750` by default. A reverse proxy sits between the internet and Core to provide:

- TLS termination (HTTPS) with automatic certificate renewal via Let's Encrypt
- HTTP-to-HTTPS redirection
- Standard ports (80/443) without running Core as root
- Additional security headers and access control

Core can terminate TLS directly (see the `network.tls` section in [configuration.md](configuration.md)), but using a reverse proxy is simpler for Let's Encrypt certificate renewal.

## nginx

The repository ships an nginx config template at `deploy/nginx/life-engine.conf`.

### Install nginx

```bash
# Debian/Ubuntu
sudo apt update && sudo apt install -y nginx

# RHEL/Fedora
sudo dnf install -y nginx
```

### Install the config

```bash
sudo cp deploy/nginx/life-engine.conf /etc/nginx/sites-available/life-engine
sudo ln -s /etc/nginx/sites-available/life-engine /etc/nginx/sites-enabled/
```

### Edit the domain name

Open `/etc/nginx/sites-available/life-engine` and replace `life-engine.example.com` with your actual domain on these lines:

- `server_name life-engine.example.com;` (appears in both server blocks)
- `ssl_certificate /etc/letsencrypt/live/life-engine.example.com/fullchain.pem;`
- `ssl_certificate_key /etc/letsencrypt/live/life-engine.example.com/privkey.pem;`

### Config structure

The config defines two server blocks:

- **Port 80** -- Redirects all HTTP traffic to HTTPS with a `301` redirect.
- **Port 443** -- Terminates TLS and proxies all requests to Core via the `life_engine_core` upstream (`127.0.0.1:3750`).

The full config from `deploy/nginx/life-engine.conf`:

```nginx
upstream life_engine_core {
    server 127.0.0.1:3750;
}

server {
    listen 80;
    server_name life-engine.example.com;

    # Redirect HTTP to HTTPS
    return 301 https://$host$request_uri;
}

server {
    listen 443 ssl http2;
    server_name life-engine.example.com;

    # TLS — replace with your certificate paths or use Let's Encrypt
    ssl_certificate     /etc/letsencrypt/live/life-engine.example.com/fullchain.pem;
    ssl_certificate_key /etc/letsencrypt/live/life-engine.example.com/privkey.pem;
    ssl_protocols       TLSv1.2 TLSv1.3;
    ssl_ciphers         HIGH:!aNULL:!MD5;

    location / {
        proxy_pass http://life_engine_core;
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;

        # SSE / WebSocket support
        proxy_http_version 1.1;
        proxy_set_header Connection "";
        proxy_buffering off;
        proxy_cache off;
        proxy_read_timeout 86400s;
    }
}
```

### SSE, buffering, and timeout settings explained

Life Engine Core uses Server-Sent Events (SSE) for real-time communication. The following nginx directives are required for SSE to work correctly through the proxy:

- `proxy_http_version 1.1` and `proxy_set_header Connection ""` -- SSE requires HTTP/1.1 with keep-alive connections. Setting `Connection` to an empty string enables keep-alive between nginx and Core, which is necessary for the persistent SSE connection to remain open.

- `proxy_buffering off` -- By default, nginx buffers responses from the upstream before sending them to the client. This breaks SSE because events are held in the buffer instead of being delivered immediately. Disabling buffering ensures each event is forwarded to the client the moment Core sends it.

- `proxy_cache off` -- Prevents nginx from caching SSE responses. Cached event streams would deliver stale data or no data at all.

- `proxy_read_timeout 86400s` -- SSE connections are long-lived (potentially open for hours or days). The default nginx read timeout of 60 seconds would close idle SSE connections. Setting this to 86400 seconds (24 hours) allows connections to survive without nginx timing them out.

### Obtain a Let's Encrypt certificate with Certbot

Install Certbot and obtain a certificate before testing the nginx config:

```bash
# Debian/Ubuntu
sudo apt install -y certbot python3-certbot-nginx

# Obtain the certificate (nginx must be running on port 80)
sudo certbot --nginx -d life-engine.example.com
```

Certbot modifies the nginx config to point to the correct certificate paths and sets up automatic renewal via a systemd timer.

### Test and reload

```bash
sudo nginx -t && sudo systemctl reload nginx
```

## Caddy

The repository ships a Caddy config at `deploy/caddy/Caddyfile`. Caddy handles TLS automatically via Let's Encrypt when a public domain is used, so no manual certificate management is needed.

### Install Caddy

```bash
# Debian/Ubuntu
sudo apt install -y debian-keyring debian-archive-keyring apt-transport-https
curl -1sLf 'https://dl.cloudsmith.io/public/caddy/stable/gpg.key' | sudo gpg --dearmor -o /usr/share/keyrings/caddy-stable-archive-keyring.gpg
curl -1sLf 'https://dl.cloudsmith.io/public/caddy/stable/debian.deb.txt' | sudo tee /etc/apt/sources.list.d/caddy-stable.list
sudo apt update && sudo apt install -y caddy
```

### Install the config

```bash
sudo cp deploy/caddy/Caddyfile /etc/caddy/Caddyfile
```

### Edit the domain name

Open `/etc/caddy/Caddyfile` and replace `life-engine.example.com` with your actual domain:

```text
your-domain.example.com {
    reverse_proxy localhost:3750
}
```

The full default config from `deploy/caddy/Caddyfile`:

```text
life-engine.example.com {
    reverse_proxy localhost:3750
}
```

### How Caddy handles TLS

When you use a public domain name in the Caddyfile, Caddy automatically:

- Obtains a Let's Encrypt certificate for the domain
- Redirects HTTP (port 80) to HTTPS (port 443)
- Renews the certificate before it expires

No additional TLS configuration is needed. Caddy stores certificates in its data directory (typically `/var/lib/caddy/.local/share/caddy/`).

### How Caddy handles SSE

Caddy supports SSE connections without any special configuration. It does not buffer responses by default and keeps connections alive, so SSE works out of the box with the simple `reverse_proxy` directive.

### Reload

```bash
sudo systemctl reload caddy
```

## TLS with self-signed certificates

If you do not have a public domain (for example, on a LAN), you can generate a self-signed certificate for nginx:

```bash
sudo openssl req -x509 -nodes -days 365 -newkey rsa:2048 \
  -keyout /etc/ssl/private/life-engine.key \
  -out /etc/ssl/certs/life-engine.crt \
  -subj "/CN=life-engine.local"
```

Then update the nginx config to point to these paths instead of the Let's Encrypt paths:

- `ssl_certificate /etc/ssl/certs/life-engine.crt;`
- `ssl_certificate_key /etc/ssl/private/life-engine.key;`

For Caddy on a local network, use the `tls internal` directive:

```text
life-engine.local {
    tls internal
    reverse_proxy localhost:3750
}
```

This tells Caddy to use its built-in CA to issue a locally-trusted certificate.

## Docker and reverse proxy

The same nginx and Caddy configurations work when Core runs in Docker with port `3750` published to the host. The proxy connects to `localhost:3750` regardless of whether Core is a bare-metal process or a container.

If the reverse proxy also runs in Docker, use Docker networking instead of `localhost`. For example, if both containers share a Docker network, replace `127.0.0.1:3750` in the nginx upstream (or `localhost:3750` in the Caddyfile) with the Core container's service name (for example, `core:3750`).

# DNS SRV Records for CalDAV/CardDAV Service Discovery

Native calendar and contacts clients use DNS SRV records to auto-discover CalDAV and CardDAV services when a user enters their email address or domain. This document explains how to configure DNS SRV records for a Life Engine deployment on a custom domain.

## Prerequisites

- A domain name you control (e.g. `example.com`)
- Life Engine Core running and accessible via HTTPS
- Access to your domain's DNS management panel

## Required SRV Records

Add the following DNS records for your domain. Replace `core.example.com` with the hostname where Life Engine Core is running.

### CalDAV (Calendar)

- **Record type** — `SRV`
- **Name** — `_caldavs._tcp.example.com`
- **Priority** — `0`
- **Weight** — `1`
- **Port** — `443`
- **Target** — `core.example.com`

For non-TLS (development only):

- **Name** — `_caldav._tcp.example.com`
- **Port** — `80` (or your Core HTTP port)

### CardDAV (Contacts)

- **Record type** — `SRV`
- **Name** — `_carddavs._tcp.example.com`
- **Priority** — `0`
- **Weight** — `1`
- **Port** — `443`
- **Target** — `core.example.com`

For non-TLS (development only):

- **Name** — `_carddav._tcp.example.com`
- **Port** — `80` (or your Core HTTP port)

## Required TXT Records

Some clients use TXT records to locate the context path:

- **Name** — `_caldavs._tcp.example.com`
- **Value** — `path=/api/plugins/com.life-engine.api-caldav/calendars/`

- **Name** — `_carddavs._tcp.example.com`
- **Value** — `path=/api/plugins/com.life-engine.api-carddav/addressbooks/`

## Well-Known URLs

Life Engine Core also serves `.well-known` endpoints (RFC 6764) for clients that use HTTP-based discovery instead of DNS:

- `https://core.example.com/.well-known/caldav` redirects to the CalDAV principal URL
- `https://core.example.com/.well-known/carddav` redirects to the CardDAV principal URL

These work automatically when the `api-caldav` and `api-carddav` plugins are loaded. No additional configuration is needed.

## Client Configuration

### iOS Calendar / Contacts

1. Open Settings > Calendar > Accounts > Add Account > Other
2. Select "Add CalDAV Account" or "Add CardDAV Account"
3. Enter server: `core.example.com`, username, and password
4. iOS uses DNS SRV + `.well-known` to discover the service automatically

### Thunderbird

1. Open the address book
2. File > New > Remote Address Book (CardDAV)
3. Enter URL: `https://core.example.com/.well-known/carddav`
4. Thunderbird follows the redirect to discover the address book

For calendar:

1. Open the calendar view
2. File > New Calendar > On the Network
3. Select CalDAV and enter: `https://core.example.com/.well-known/caldav`

### GNOME Calendar / Contacts

1. Open Settings > Online Accounts > Other
2. Select CalDAV or CardDAV
3. Enter the server URL: `core.example.com`
4. GNOME uses DNS SRV + `.well-known` for discovery

## Verification

Test your DNS SRV records with:

```bash
dig SRV _caldavs._tcp.example.com
dig SRV _carddavs._tcp.example.com
```

Test `.well-known` endpoints with:

```bash
curl -I https://core.example.com/.well-known/caldav
curl -I https://core.example.com/.well-known/carddav
```

Both should return a `301` redirect to the respective principal URL.

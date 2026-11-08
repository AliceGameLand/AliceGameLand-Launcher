# Download delivery operations

Public delivery endpoint: https://alice-launcher-delivery.gptnr2140.workers.dev
Worker source: `scripts/delivery-worker.mjs` (Cloudflare Worker `alice-launcher-delivery`, account subdomain `gptnr2140`). GitHub authentication uses the GITHUB_TOKEN Worker secret. The token is never included in launcher files or source code, and no player credentials are used.

Only GET/HEAD for the two AliceGameLand repositories' release metadata and release assets are allowed. Other repositories and arbitrary upstream URLs are rejected. Metadata is cached briefly; successful asset responses are cached and errors are not. Asset API requests include a short time bucket to avoid stale rate-limit responses. The launcher retries through delivery/original paths and stores downloads in temporary files before replacement. Hash validation remains enabled.

New releases remain on GitHub. The relay discovers releases dynamically; future releases do not require an endpoint change. Do not embed a GitHub access token in the launcher or Worker source.

Validation on 2026-11-08: all 191 unique distribution downloads passed size/hash checks. All six server configurations passed isolated module validation and loader-manifest loading. Actual game login, rendering and server entry were not run. Automatic update download and install-on-quit remain disabled.

Outages at both providers can still prevent downloads. A failed download leaves the prior file intact and can be retried; failure reports include the filename and status without signed download URLs.

## Server authentication

The dedicated token is registered as a Cloudflare secret_text binding named GITHUB_TOKEN. Deployment must preserve secret_text bindings (keep_bindings). Authorization is sent only to api.github.com; asset redirects are validated and followed without the token. No public endpoint exposes the secret.

Authenticated core API limit verified: 5,000 requests/hour, shared with the token owner. Current token expiration: 2027-02-06 00:00 Asia/Seoul. Rotate the server secret before expiration; no launcher reinstall is required.

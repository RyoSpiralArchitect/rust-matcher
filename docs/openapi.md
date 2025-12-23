# API documentation plan

This repository does not yet publish an OpenAPI document, but the API surface is small enough to keep a hand-written specification in sync until a generator is wired in. The immediate next steps are:

- Define a minimal `openapi.yaml` that documents the public endpoints already in service (`/livez`, `/readyz`, `/api/queue/*`, `/api/projects/{project_id}/candidates`, `/api/feedback`).
- Serve that document at `/openapi.json` (or `/openapi.yaml`) and mount Swagger UI at `/docs` so client developers can explore request/response shapes.
- Ensure authentication headers (`X-API-Key` or `Authorization: Bearer ...`) are called out per route and that admin-only operations such as `/api/queue/retry/:id` and source-text includes are flagged with the correct scopes/roles.

Until the served document exists, keep this file as the canonical description of what should be reflected in the OpenAPI spec:

- **Authentication**: API key via `X-API-Key` by default; JWT mode is supported when configured. Admin-only routes: queue retry, source text includes.
- **Health**: `/livez` (process liveness), `/readyz` and `/health` (readiness + DB ping, returns 503 when not ready).
- **Queue**:
  - `GET /api/queue/dashboard` – requires auth, returns dashboard counts.
  - `GET /api/queue/jobs` – list with pagination (`limit`, `offset`) and filters (`status`, `final_method`).
  - `GET /api/queue/jobs/{id}` – optional `include` flags, `limit`, `days` (bounded to 365) for match/feedback history.
  - `POST /api/queue/retry/{id}` – admin only.
- **Candidates**: `GET /api/projects/{project_id}/candidates` with `limit`/`offset` and optional `include_softko` flag; results are de-duplicated by match result.
- **Feedback**: `POST /api/feedback` accepts a JSON body (request size limited by default body limit) and uses the authenticated user as the actor.

Follow-up tasks: keep the OpenAPI spec checked into `docs/openapi.yaml`, add CI checks to ensure it stays valid, and expose `/docs` for interactive browsing.

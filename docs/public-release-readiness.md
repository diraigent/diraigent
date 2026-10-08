# Public release checks

Use a feature branch and run the `Validate` workflow before merging. Binary
releases also depend on it: API tests (with PostgreSQL required), Clippy,
production web build, permission/navigation browser tests, and iOS unit tests.
Local tests may skip an unavailable database; release CI sets
`REQUIRE_TEST_DATABASE=true` so a missing database cannot pass the integration gate.

## Access and demo data

Test viewer, editor and manager credentials independently. Verify hidden-project
reads, writes, grants, revocation, and workspace selection. Shared demo accounts
must have only the intended project grant. Review inherited manager grants after
upgrading an existing installation; do not remove them without the owners' agreement.

Review all content visible to the demo, including audit snapshots, stored diffs,
logs, context, comments, reports and repository files. Read-only access is not
redaction. Pattern scans can flag credentials and private infrastructure but
cannot establish that arbitrary text is safe to publish. Repeat the review when
real project data changes, or use a dedicated synthetic workspace.

## Logs and monitoring

API request spans and web access logs record paths without query strings or
referrers. Ensure upstream proxies do not separately retain OAuth callback URLs.
Keep historical logs private and apply a retention policy. Application diagnostic
messages and nginx error logs still require review: avoiding access-log query
strings alone does not redact arbitrary diagnostic text.

The portable files in `ops/monitoring` provision a Prometheus-backed Grafana
dashboard and alerts for missing API telemetry, server errors and sustained p95
latency above two seconds. The API exports `service.name=diraigent-api`; duration
buckets use seconds with millisecond-scale boundaries. Keep host addresses,
deployment mounts, contact points and their credentials in private configuration.
The missing-telemetry alert uses the periodic health requests; it does not prove
public DNS, proxy routing or OAuth availability.

Grafana file provisioning follows the [official provisioning instructions](https://grafana.com/docs/grafana/latest/alerting/set-up/provision-alerting-resources/file-provisioning/).
Mount the dashboard at `/etc/diraigent-monitoring/dashboard.json`, the dashboard
provider under `provisioning/dashboards`, and the alert file under
`provisioning/alerting`. Configure and test a notification destination before
relying on unattended alert delivery.

## Deployment and recovery

Take a private database backup before deployment. Restore it into a separate
database and verify migrations, identities and project/task counts. Never restore
over production to test a backup. Preserve committed migration files; corrections
must use new migrations. Include OAuth configuration, encryption keys and private
deployment/provider configuration in the recovery plan, with restricted access.

After deployment, test an actual OAuth login, workspace creation/selection, worker
connection, a task through completion, and project chat. On a physical iPhone,
keep a session and chat open across access-token expiry, then exercise background
and foreground transitions and a temporary network outage. Simulator unit tests
cover refresh logic but do not replace the physical-device sign-in check.

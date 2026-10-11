# OpenCode onboarding

## Local Docker Compose

Install Git, Docker with Compose, Bash, curl and Python 3. The helper runs on Linux and
macOS and uses OpenCode by default. Claude Code and Codex are optional task
providers; no Claude installation, subscription or macOS Keychain is required.

Clone the repository and use its `startup/` directory so the image builds have
all sources available. Copy `.env.example` to `.env`.

```bash
git clone https://github.com/diraigent/diraigent.git
cd diraigent/startup
cp .env.example .env
chmod +x start.sh
```

Run `./start.sh --build` on first startup or after updating the source. It builds
local API, web and Orchestra images; it does not depend on Docker Hub's `latest`
tags. You can combine `--build` with `--login` for the first interactive login.

Choose one authentication method:

- Set a provider API key in `.env`, such as `OPENAI_API_KEY` or
  `ANTHROPIC_API_KEY`.
- Run `./start.sh --build --login` on first use, or `./start.sh --login` later, to connect an account interactively through OpenCode.
  Its API-key/OAuth login is saved in the persistent OpenCode volume. This command
  does not start the API or register an Orchestra.

Run `./start.sh --models` to list model IDs, then set
`OPENCODE_MODEL=provider/model` in `.env`. Choose a model your connected account
can use. Startup requires an explicit model and a connection for that provider;
it does not make a paid inference request or prove that your account can use the
selected model. Check project chat after startup.

```bash
./start.sh --build
```

Startup initializes worker volumes, checks the selected provider connection,
starts PostgreSQL and the API, waits for readiness, creates an owned workspace
if the user has none, and registers an Orchestra.
The API assigns its workspace role. The helper saves the one-time agent API key
and ID in an owner-only `.env` file without printing them. Subsequent starts
verify and reuse that identity. An inaccessible identity is an error rather than
an instruction to register a replacement; it may own durable project content.

Open `http://localhost:4200`, create a project with its repository URL and default
branch, verify project chat, then make a task ready. Configure Git credentials for
remote delivery. `GIT_REPO_URL` is not used by the helper; repository URLs belong
in project settings.

The example uses development authentication and binds its ports to localhost.
`API_PORT`, `WEB_PORT` and `POSTGRES_PORT` can change the default ports. For OIDC,
remove `DEV_USER_ID`, configure the API/web identity-provider settings, and supply
`STARTUP_API_TOKEN` with a user access token for initial registration. The helper
removes that bootstrap token from `.env` after saving the agent key. Alternatively,
create the agent in the web app and set both `AGENT_ID` and
`DIRAIGENT_API_TOKEN`. These are Diraigent credentials, separate from model-provider
credentials.

## Storage and upgrades

Compose persists four named volumes:

| Volume | Contents |
| --- | --- |
| `pgdata` | PostgreSQL shared state and permissions |
| `projects` | Worker repositories and task worktrees |
| `orchestra-data` | Orchestra runtime data, logs and `project-content.db` |
| `opencode-data` | OpenCode state, saved accounts and provider sessions |

The worker runs as UID/GID 1001. A one-shot initialization service prepares the
volume mount roots for that user. Container replacement preserves these volumes;
`docker compose down -v` deletes them. Keep the same Compose project name when
upgrading so it reuses the same volumes. Keep credentials and private deployments
out of version control.

The PostgreSQL 18 volume now mounts at `/var/lib/postgresql`, matching the official
image's version-specific data directory. For an existing installation with a
custom or older mount, back up and inspect its data layout before changing mounts;
the helper does not rearrange an existing database. See the
[official PostgreSQL image guidance](https://github.com/docker-library/docs/blob/master/postgres/README.md#pgdata).

Enable full storage per project after assigning an updated Orchestra owner.
Back up its content database together with PostgreSQL; see
[Orchestra storage](orchestra-project-storage.md).

## Host workers and other providers

For a native Orchestra, copy `apps/orchestra/.env.example` to `.env`, configure
its Diraigent agent credentials, persistent `DATA_DIR`, and `OPENCODE_MODEL`.
Run `opencode auth login` on that host or supply the model provider's supported
API-key environment variable. OpenCode credentials on your laptop are not
automatically shared with a worker on another host or in a container.

For a container connected to an existing API, use `startup/orchestra.yml`.
Build the worker with `docker compose -f orchestra.yml build orchestra`.
Initialize its volumes with `docker compose -f orchestra.yml run --rm orchestra-init`,
then authenticate with
`docker compose -f orchestra.yml run --rm --no-deps --entrypoint opencode orchestra auth login --standalone`.
Set `OPENCODE_MODEL` and both Diraigent agent credentials before starting it.

Select optional task providers through `context.worker.provider`, such as
`codex` or `claude-code`, and configure their credentials separately. OpenCode
itself can use several model providers, so using an Anthropic model through
OpenCode does not require the Claude Code CLI.

OpenCode v2 stores saved accounts in SQLite and reads supported API-key variables
from its server process. Orchestra invokes private standalone servers so container
environment settings apply to each task. See
[OpenCode authentication](https://opencode.ai/v2/docs/cli/providers/).

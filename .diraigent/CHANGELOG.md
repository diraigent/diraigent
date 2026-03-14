# Changelog


## v20260314-2332 (2026-03-14)

- Renamed Goals to Work across the entire stack (database, API, orchestra, web frontend)
- Added AI-powered task planning: Plan Tasks button on Work view generates tasks via orchestra
- Work item planning now routes through orchestra WebSocket instead of direct API calls
- Auto-generate success criteria for work items when empty
- Chat history compression now uses claude subprocess instead of direct Anthropic API
- Added version info section to Account Settings
- Fixed mobile horizontal scrolling issues
- Fixed error display showing [object Object] on git push failures
- Fixed task transition to human_review on merge failure
- Improved task detail view layout (removed nested scrolling)
- Updated container builds for API, Orchestra, and Web

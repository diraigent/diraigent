# Changelog

## v20260315-0838 (2026-03-15)
- Renamed Goals to Work across the entire application
- Added AI-powered Plan Tasks feature for work items with preview dialog
- Routed work item planning through orchestra for improved reliability
- Fixed auth token handling in API
- Fixed empty planning responses with better error diagnostics
- Tasks now transition to human review on merge failure
- Moved task statistics section above linked tasks for better visibility
- Fixed mobile horizontal scrolling across all pages
- Redesigned landing page with new screenshots
- Added Playwright end-to-end test infrastructure
- Improved SSE service reliability for agent status and reviews

---


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

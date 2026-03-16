-- base_url is now the full repository URL (e.g. https://github.com/owner/repo)
-- rather than the API base URL. Remove the old default.
ALTER TABLE diraigent.github_integration
    ALTER COLUMN base_url DROP DEFAULT;

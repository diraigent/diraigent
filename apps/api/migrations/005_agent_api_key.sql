-- Agent API keys: store a SHA-256 hash of the key on the agent row.
-- The plaintext key (dak_...) is returned only once at registration time.
ALTER TABLE diraigent.agent ADD COLUMN api_key_hash text;
CREATE UNIQUE INDEX idx_agent_api_key_hash ON diraigent.agent (api_key_hash) WHERE api_key_hash IS NOT NULL;

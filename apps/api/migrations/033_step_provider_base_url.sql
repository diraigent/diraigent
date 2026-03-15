-- Add provider and base_url fields to step_template for multi-provider support.
-- provider: e.g. "anthropic", "openai", "ollama". NULL defaults to "anthropic".
-- base_url: override the default API endpoint for the chosen provider.

ALTER TABLE diraigent.step_template
    ADD COLUMN provider text,
    ADD COLUMN base_url text;

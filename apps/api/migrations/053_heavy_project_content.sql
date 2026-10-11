-- Explicit activation after the owner reports content protocol 2.
ALTER TABLE diraigent.project_content_owner
    ADD COLUMN heavy_content boolean NOT NULL DEFAULT false;
ALTER TABLE diraigent.project_content_ref
    DROP CONSTRAINT project_content_ref_kind_check;
ALTER TABLE diraigent.project_content_ref
    ADD CONSTRAINT project_content_ref_kind_check
    CHECK (kind IN ('log','diff','artifact','field'));
-- Vectors cannot carry inline text references, so keep their routing separately.
CREATE TABLE diraigent.knowledge_embedding_ref (
    knowledge_id uuid PRIMARY KEY REFERENCES diraigent.knowledge(id) ON DELETE CASCADE,
    object_id uuid NOT NULL,
    checksum text NOT NULL,
    bytes bigint NOT NULL CHECK (bytes >= 0)
);

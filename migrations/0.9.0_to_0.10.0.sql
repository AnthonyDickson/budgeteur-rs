ALTER TABLE "transaction"
    ADD tag_id INTEGER REFERENCES tag(id) ON UPDATE CASCADE ON DELETE SET NULL;

CREATE INDEX IF NOT EXISTS idx_transaction_date_tag
    ON "transaction"(date, tag_id);

DROP TABLE transaction_tag;

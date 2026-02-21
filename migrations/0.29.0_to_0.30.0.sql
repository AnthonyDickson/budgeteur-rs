BEGIN TRANSACTION;

CREATE TABLE IF NOT EXISTS untagged_transaction (
    transaction_id INTEGER PRIMARY KEY,
    created_at TEXT NOT NULL,
    FOREIGN KEY(transaction_id) REFERENCES "transaction"(id) ON DELETE CASCADE
);

CREATE TRIGGER IF NOT EXISTS remove_from_untagged_queue_on_tag_set
AFTER UPDATE OF tag_id ON "transaction"
WHEN NEW.tag_id IS NOT NULL
BEGIN
    DELETE FROM untagged_transaction WHERE transaction_id = NEW.id;
END;

COMMIT;

# Technical Specification: Imported Untagged Transactions Queue

## Purpose

Define the design and behavior for a queue of imported transactions that remain untagged, allowing a fast manual tagging flow without polluting the queue with manually created or re-untagged transactions.

## Goals

- Track untagged transactions created during CSV import only.
- Remove items from the queue as soon as a tag is set (non-NULL).
- Do not add manually created transactions to the queue.
- Do not re-add transactions to the queue if their tag is removed later.

## Non-Goals

- Represent every `transaction` with `tag_id IS NULL`.
- Provide a full audit history of tagging actions.
- Track tagging states for manually created transactions.

## Data Model

### New Table: `untagged_transaction`

- `transaction_id INTEGER PRIMARY KEY`
- `created_at TEXT NOT NULL` (UTC ISO-8601 timestamp, e.g. `2026-02-21T14:23:05Z`)
- Foreign key: `transaction_id REFERENCES "transaction"(id) ON DELETE CASCADE`

Rationale:
- Primary key ensures a single queue row per transaction.
- `created_at` supports ordering the queue by most recently added.
- `ON DELETE CASCADE` makes queue cleanup automatic when a transaction is deleted.

## Lifecycle Rules

### Insert

- A row is inserted into `untagged_transaction` only during CSV import.
- Only inserted when the imported transaction is newly created and `tag_id IS NULL`.
- If auto-tagging has already set `tag_id` during the import flow, skip insertion.
- Duplicates from re-imports are ignored since no new transaction row is created.
- `created_at` is set at insert time during import.

### Removal

- When a transaction’s `tag_id` transitions to a non-NULL value, the queue entry is removed.
- If `tag_id` is later set back to NULL, the queue entry is **not** re-created.

### Manual Transactions

- Manually created transactions are never added to the queue (even if untagged).

## Triggers

### `remove_from_untagged_queue_on_tag_set`

- Fires `AFTER UPDATE OF tag_id` on `transaction`.
- If `NEW.tag_id IS NOT NULL`, delete matching row from `untagged_transaction`.
- No insertion logic in triggers (insertions only happen during import).

This ensures the queue only ever shrinks outside of imports, avoiding accidental re-adding.

## Queries

- Queue view: `SELECT ... FROM "transaction" JOIN untagged_transaction ... ORDER BY untagged_transaction.created_at DESC`.
- Default page size is 20 items.
- The page shows at most 20 items at a time; fetching more requires applying the current batch.
- Optional filtering by import date can be applied using `transaction.date` or related metadata in the future.
- Tie-breaker: when `created_at` is equal, order by `transaction_id` DESC for stability.

## UI Notes

- The quick-tagging UI uses per-row tag chips (hidden inputs + pill labels) to minimize clicks.
- Changes are applied in a single batch submit; dismissed rows are removed only on apply.
- After a successful apply, fetch and render the next batch of untagged transactions.
- When the queue is empty (initially or after apply), show a full-page empty state: "All done, no untagged transactions left" with a link back to the transactions page/import page.
- A shared tag palette (single selection control applied to the focused row) is a potential future enhancement.
- Layout uses cards (not a table). Description is the primary line, with date + amount in a single meta row.
- Batch form shape: `tag_id_<transaction_id>=<tag_id>` and `dismiss=<transaction_id>` (repeatable).
- Validation errors are shown via a top-level alert.
- Dismiss removes the transaction from the queue without changing its tag.
- On success, show a confirmation alert such as "Applied tags to X transactions, dismissed Y".
- Tag and dismiss inputs are mutually exclusive via a small page script (selecting one clears the other).

## Migration & Backfill

- No backfill is planned; the queue will only include transactions from imports after this change.

## Edge Cases

- If auto-tagging runs immediately after import, it can remove queue entries. This is acceptable.
- Bulk tagging updates rely on the trigger for queue cleanup.
- Tag removal (setting `tag_id` to NULL) does not re-add to queue by design.
- Queue insertion and transaction import run in a single DB transaction for atomicity.

## Source of Truth

- Queue membership is authoritative in `untagged_transaction`.
- `transaction.tag_id` remains the source of truth for tagging state.

## Future Considerations

- Add `import_batch` if per-import grouping is needed later.

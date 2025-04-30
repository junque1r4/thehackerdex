### Task 3: Implement Database Schema for Trace Linking (Option B)

* **Goal:** Create a dedicated database table to store explicit relationships between addresses discovered during forward/backward tracing, enabling graph visualization.

* **Major Task 3.1: Define `trace_links` Table Schema**
    * **Sub-task 3.1.1:** Design SQL `CREATE TABLE` statement for `trace_links`.
        * Include columns:
            * `id` (SERIAL PRIMARY KEY)
            * `source_address` (TEXT, NOT NULL, FK referencing `known_addresses.address`)
            * `target_address` (TEXT, NOT NULL, FK referencing `known_addresses.address`)
            * `relationship_type` (TEXT NOT NULL) - e.g., 'forward_trace_hop', 'backward_trace_hop'
            * `trace_initiator` (TEXT NOT NULL, FK referencing `known_addresses.address`)
            * `hop_count` (INTEGER NOT NULL)
            * `discovery_timestamp` (TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP)
            * `transaction_signature` (TEXT NULL)
    * **Sub-task 3.1.2:** Add necessary indexes (e.g., on `trace_initiator`, `source_address`, `target_address`).
    * **Sub-task 3.1.3:** Create a new SQL migration file (using `sqlx-cli` if applicable, e.g., `sqlx migrate add create_trace_links_table`) and add the `CREATE TABLE` statement.

* **Major Task 3.2: Update Database Models in Rust**
    * **Sub-task 3.2.1:** Create a new Rust struct in `src/db/models.rs` to represent the `trace_links` table (e.g., `TraceLink`). Ensure it derives necessary traits (`Debug`, `Clone`, `Serialize`, `Deserialize`, `sqlx::FromRow`).

* **Major Task 3.3: Implement Database Repository Functions for `trace_links`**
    * **Sub-task 3.3.1:** Add a new function in `src/db/repository.rs` to insert a `TraceLink` record (e.g., `add_trace_link(pool: &PgPool, link_data: &TraceLinkData) -> Result<(), HackerdexError>`). Define a `TraceLinkData` struct if needed, omitting the `id` and potentially `discovery_timestamp`.
    * **Sub-task 3.3.2:** (Optional) Add functions to query `trace_links` based on `trace_initiator`, `source_address`, `target_address`, or `relationship_type` as needed for future analysis or visualization tools.

* **Major Task 3.4: Integrate `trace_links` Insertion into Tracer Logic**
    * **Sub-task 3.4.1:** Modify Forward Tracking (Task 1.3 & 1.4): When an outgoing transaction from a traced address (Source) to a filtered Recipient is identified:
        * Determine the `trace_initiator` (the original high-risk address).
        * Determine the `hop_count`.
        * Call `add_trace_link` with `source_address` = Source, `target_address` = Recipient, `relationship_type` = 'forward_trace_hop', `trace_initiator`, `hop_count`, and the relevant `transaction_signature`.
    * **Sub-task 3.4.2:** Modify Backward Tracking (Task 2.3): When an incoming transaction from a filtered Source to a traced address (Destination) is identified:
        * Determine the `trace_initiator` (the original high-risk destination).
        * Determine the `hop_count` (always 1 in this simple backward case, unless multi-hop backward tracing is added).
        * Call `add_trace_link` with `source_address` = Source, `target_address` = Destination, `relationship_type` = 'backward_trace_hop', `trace_initiator`, `hop_count`, and the `transaction_signature`.
    * **Sub-task 3.4.3:** Ensure `source_address` and `target_address` involved in the link are added to the `known_addresses` table *before* attempting to insert into `trace_links` (due to Foreign Key constraints). The logic in Task 1.5 and 2.4 already handles adding addresses, ensure it runs first.

* **Major Task 3.5: Run Database Migrations**
    * **Sub-task 3.5.1:** Apply the new migration using `sqlx-cli` (`sqlx migrate run`) or ensure it's run automatically on application startup (like in `src/bin/db_check.rs` or `src/main.rs`).

---

use super::models::{AddressData, AddressRecord, TraceLink, TraceLinkData};
use crate::error::HackerdexError;
use sqlx::types::time::OffsetDateTime;
use sqlx::{PgPool, postgres::PgQueryResult};

/// Repository struct provides a wrapper around database access functions
#[derive(Clone)]
pub struct Repository {
    pub pool: PgPool,
}

impl Repository {
    /// Create a new repository instance
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Get details for a specific address
    pub async fn get_address_details(
        &self,
        address: &str,
    ) -> Result<AddressRecord, HackerdexError> {
        get_address_details(&self.pool, address).await
    }

    /// Add a new known address to the database
    pub async fn add_known_address(
        &self,
        address_data: &AddressData,
    ) -> Result<(), HackerdexError> {
        add_known_address(&self.pool, address_data).await
    }

    /// Update details for an existing address
    pub async fn update_address_details(
        &self,
        address_data: &AddressData,
    ) -> Result<(), HackerdexError> {
        update_address_details(&self.pool, address_data).await
    }

    /// Get all addresses by category
    pub async fn get_all_addresses_by_category(
        &self,
        category: &str,
    ) -> Result<Vec<AddressRecord>, HackerdexError> {
        get_all_addresses_by_category(&self.pool, category).await
    }

    /// Get addresses by multiple criteria (categories and risk levels)
    pub async fn get_addresses_by_criteria(
        &self,
        categories: &[String],
        risk_levels: &[String],
    ) -> Result<Vec<AddressRecord>, HackerdexError> {
        get_addresses_by_criteria(&self.pool, categories, risk_levels).await
    }

    /// Add a new trace link to the database
    pub async fn add_trace_link(&self, link_data: &TraceLinkData) -> Result<(), HackerdexError> {
        add_trace_link(&self.pool, link_data).await
    }

    /// Get trace links for a specific initiator address
    pub async fn get_trace_links_by_initiator(
        &self,
        initiator: &str,
    ) -> Result<Vec<TraceLink>, HackerdexError> {
        get_trace_links_by_initiator(&self.pool, initiator).await
    }

    /// Get trace links for a specific source address
    pub async fn get_trace_links_by_source(
        &self,
        source_address: &str,
    ) -> Result<Vec<TraceLink>, HackerdexError> {
        get_trace_links_by_source(&self.pool, source_address).await
    }

    /// Get trace links for a specific target address
    pub async fn get_trace_links_by_target(
        &self,
        target_address: &str,
    ) -> Result<Vec<TraceLink>, HackerdexError> {
        get_trace_links_by_target(&self.pool, target_address).await
    }

    /// Get trace links by relationship type
    pub async fn get_trace_links_by_relationship_type(
        &self,
        relationship_type: &str,
    ) -> Result<Vec<TraceLink>, HackerdexError> {
        get_trace_links_by_relationship_type(&self.pool, relationship_type).await
    }
}

/// Initialize database connection and run migrations
pub async fn initialize_db(pool: &PgPool) -> Result<(), HackerdexError> {
    // Just test database connectivity first
    match sqlx::query!("SELECT 1 as connected").fetch_one(pool).await {
        Ok(_) => {
            // Database connection is good, migrations will be handled separately
            Ok(())
        }
        Err(e) => Err(HackerdexError::DatabaseError(format!(
            "Failed to connect to database: {}",
            e
        ))),
    }
}

/// Add a new known address to the database
pub async fn add_known_address(
    pool: &PgPool,
    address_data: &AddressData,
) -> Result<(), HackerdexError> {
    let result = sqlx::query!(
        r#"
        INSERT INTO known_addresses 
            (address, entity_name, category, risk_level, source_of_info, confidence_score, notes)
        VALUES 
            ($1, $2, $3, $4, $5, $6, $7)
        "#,
        address_data.address,
        address_data.entity_name,
        address_data.category,
        address_data.risk_level,
        address_data.source_of_info,
        address_data.confidence_score,
        address_data.notes
    )
    .execute(pool)
    .await;

    match result {
        Ok(_) => Ok(()),
        Err(e) => Err(HackerdexError::DatabaseError(format!(
            "Failed to add address: {}",
            e
        ))),
    }
}

/// Get details for a specific address
pub async fn get_address_details(
    pool: &PgPool,
    address: &str,
) -> Result<AddressRecord, HackerdexError> {
    let record = sqlx::query_as!(
        AddressRecord,
        r#"
        SELECT 
            address,
            entity_name,
            category,
            risk_level,
            source_of_info,
            confidence_score,
            notes,
            created_at as "created_at!: OffsetDateTime",
            updated_at as "updated_at!: OffsetDateTime"
        FROM known_addresses WHERE address = $1
        "#,
        address
    )
    .fetch_one(pool)
    .await;

    match record {
        Ok(record) => Ok(record),
        Err(e) => Err(HackerdexError::DatabaseError(format!(
            "Failed to get address details: {}",
            e
        ))),
    }
}

/// Update details for an existing address
pub async fn update_address_details(
    pool: &PgPool,
    address_data: &AddressData,
) -> Result<(), HackerdexError> {
    let result = sqlx::query!(
        r#"
        UPDATE known_addresses
        SET 
            entity_name = $2,
            category = $3,
            risk_level = $4,
            source_of_info = $5,
            confidence_score = $6,
            notes = $7
        WHERE 
            address = $1
        "#,
        address_data.address,
        address_data.entity_name,
        address_data.category,
        address_data.risk_level,
        address_data.source_of_info,
        address_data.confidence_score,
        address_data.notes
    )
    .execute(pool)
    .await;

    match result {
        Ok(result) if result.rows_affected() == 0 => Err(HackerdexError::NotFound(format!(
            "Address {} not found",
            address_data.address
        ))),
        Ok(_) => Ok(()),
        Err(e) => Err(HackerdexError::DatabaseError(format!(
            "Failed to update address: {}",
            e
        ))),
    }
}

/// Get all addresses matching a specific category
pub async fn get_all_addresses_by_category(
    pool: &PgPool,
    category: &str,
) -> Result<Vec<AddressRecord>, HackerdexError> {
    let records = sqlx::query_as!(
        AddressRecord,
        r#"
        SELECT 
            address,
            entity_name,
            category,
            risk_level,
            source_of_info,
            confidence_score,
            notes,
            created_at as "created_at!: OffsetDateTime",
            updated_at as "updated_at!: OffsetDateTime"
        FROM known_addresses WHERE category = $1
        "#,
        category
    )
    .fetch_all(pool)
    .await;

    match records {
        Ok(records) => Ok(records),
        Err(e) => Err(HackerdexError::DatabaseError(format!(
            "Failed to get addresses by category: {}",
            e
        ))),
    }
}

/// Delete an address from the database
pub async fn delete_address(pool: &PgPool, address: &str) -> Result<PgQueryResult, HackerdexError> {
    let result = sqlx::query!("DELETE FROM known_addresses WHERE address = $1", address)
        .execute(pool)
        .await;

    match result {
        Ok(result) => Ok(result),
        Err(e) => Err(HackerdexError::DatabaseError(format!(
            "Failed to delete address: {}",
            e
        ))),
    }
}

/// Add or update an interaction between two addresses
pub async fn record_address_interaction(
    pool: &PgPool,
    source_address: &str,
    target_address: &str,
) -> Result<(), HackerdexError> {
    // First check if both addresses exist in known_addresses
    let source_exists = sqlx::query!(
        "SELECT 1 as exists FROM known_addresses WHERE address = $1",
        source_address
    )
    .fetch_optional(pool)
    .await;

    let target_exists = sqlx::query!(
        "SELECT 1 as exists FROM known_addresses WHERE address = $1",
        target_address
    )
    .fetch_optional(pool)
    .await;

    if source_exists.is_err() || target_exists.is_err() {
        return Err(HackerdexError::DatabaseError(
            "Failed to check address existence".to_string(),
        ));
    }

    if source_exists.unwrap().is_none() || target_exists.unwrap().is_none() {
        return Err(HackerdexError::NotFound(
            "One or both addresses not found in database".to_string(),
        ));
    }

    // Insert or update the interaction
    let result = sqlx::query!(
        r#"
        INSERT INTO address_interactions (source_address, target_address, interaction_count)
        VALUES ($1, $2, 1)
        ON CONFLICT (source_address, target_address) 
        DO UPDATE SET 
            interaction_count = address_interactions.interaction_count + 1,
            last_seen_at = CURRENT_TIMESTAMP
        "#,
        source_address,
        target_address
    )
    .execute(pool)
    .await;

    match result {
        Ok(_) => Ok(()),
        Err(e) => Err(HackerdexError::DatabaseError(format!(
            "Failed to record interaction: {}",
            e
        ))),
    }
}

/// Add a new trace link to the database
pub async fn add_trace_link(
    pool: &PgPool,
    link_data: &TraceLinkData,
) -> Result<(), HackerdexError> {
    // First verify that all referenced addresses exist in known_addresses
    let source_exists = sqlx::query("SELECT 1 as exists FROM known_addresses WHERE address = $1")
        .bind(&link_data.source_address)
        .fetch_optional(pool)
        .await;

    let target_exists = sqlx::query("SELECT 1 as exists FROM known_addresses WHERE address = $1")
        .bind(&link_data.target_address)
        .fetch_optional(pool)
        .await;

    let initiator_exists =
        sqlx::query("SELECT 1 as exists FROM known_addresses WHERE address = $1")
            .bind(&link_data.trace_initiator)
            .fetch_optional(pool)
            .await;

    if source_exists.is_err() || target_exists.is_err() || initiator_exists.is_err() {
        return Err(HackerdexError::DatabaseError(
            "Failed to check address existence".to_string(),
        ));
    }

    if source_exists.unwrap().is_none() {
        return Err(HackerdexError::NotFound(format!(
            "Source address not found: {}",
            link_data.source_address
        )));
    }

    if target_exists.unwrap().is_none() {
        return Err(HackerdexError::NotFound(format!(
            "Target address not found: {}",
            link_data.target_address
        )));
    }

    if initiator_exists.unwrap().is_none() {
        return Err(HackerdexError::NotFound(format!(
            "Initiator address not found: {}",
            link_data.trace_initiator
        )));
    }

    // Insert the trace link
    let query = r#"
    INSERT INTO trace_links 
        (source_address, target_address, relationship_type, trace_initiator, hop_count, transaction_signature)
    VALUES 
        ($1, $2, $3, $4, $5, $6)
    "#;

    let result = sqlx::query(query)
        .bind(&link_data.source_address)
        .bind(&link_data.target_address)
        .bind(&link_data.relationship_type)
        .bind(&link_data.trace_initiator)
        .bind(link_data.hop_count)
        .bind(&link_data.transaction_signature)
        .execute(pool)
        .await;

    match result {
        Ok(_) => Ok(()),
        Err(e) => Err(HackerdexError::DatabaseError(format!(
            "Failed to add trace link: {}",
            e
        ))),
    }
}

/// Get trace links for a specific initiator address
pub async fn get_trace_links_by_initiator(
    pool: &PgPool,
    initiator: &str,
) -> Result<Vec<TraceLink>, HackerdexError> {
    let query = r#"
    SELECT 
        id,
        source_address,
        target_address,
        relationship_type,
        trace_initiator,
        hop_count,
        discovery_timestamp,
        transaction_signature
    FROM trace_links WHERE trace_initiator = $1
    ORDER BY hop_count ASC, discovery_timestamp DESC
    "#;

    let records = sqlx::query_as::<_, TraceLink>(query)
        .bind(initiator)
        .fetch_all(pool)
        .await;

    match records {
        Ok(records) => Ok(records),
        Err(e) => Err(HackerdexError::DatabaseError(format!(
            "Failed to get trace links by initiator: {}",
            e
        ))),
    }
}

/// Get trace links by source address
#[cfg_attr(debug_assertions, allow(unused_variables))]
pub async fn get_trace_links_by_source(
    pool: &PgPool,
    source_address: &str,
) -> Result<Vec<TraceLink>, HackerdexError> {
    let records = sqlx::query_as!(
        TraceLink,
        r#"
        SELECT 
            id,
            source_address,
            target_address,
            relationship_type,
            trace_initiator,
            hop_count,
            discovery_timestamp as "discovery_timestamp!: OffsetDateTime",
            transaction_signature
        FROM trace_links WHERE source_address = $1
        ORDER BY hop_count ASC, discovery_timestamp DESC
        "#,
        source_address
    )
    .fetch_all(pool)
    .await;

    match records {
        Ok(records) => Ok(records),
        Err(e) => Err(HackerdexError::DatabaseError(format!(
            "Failed to get trace links by source: {}",
            e
        ))),
    }
}

/// Get trace links by target address
#[cfg_attr(debug_assertions, allow(unused_variables))]
pub async fn get_trace_links_by_target(
    pool: &PgPool,
    target_address: &str,
) -> Result<Vec<TraceLink>, HackerdexError> {
    let records = sqlx::query_as!(
        TraceLink,
        r#"
        SELECT 
            id,
            source_address,
            target_address,
            relationship_type,
            trace_initiator,
            hop_count,
            discovery_timestamp as "discovery_timestamp!: OffsetDateTime",
            transaction_signature
        FROM trace_links WHERE target_address = $1
        ORDER BY hop_count ASC, discovery_timestamp DESC
        "#,
        target_address
    )
    .fetch_all(pool)
    .await;

    match records {
        Ok(records) => Ok(records),
        Err(e) => Err(HackerdexError::DatabaseError(format!(
            "Failed to get trace links by target: {}",
            e
        ))),
    }
}

/// Get trace links by relationship type
#[cfg_attr(debug_assertions, allow(unused_variables))]
pub async fn get_trace_links_by_relationship_type(
    pool: &PgPool,
    relationship_type: &str,
) -> Result<Vec<TraceLink>, HackerdexError> {
    let records = sqlx::query_as!(
        TraceLink,
        r#"
        SELECT 
            id,
            source_address,
            target_address,
            relationship_type,
            trace_initiator,
            hop_count,
            discovery_timestamp as "discovery_timestamp!: OffsetDateTime",
            transaction_signature
        FROM trace_links WHERE relationship_type = $1
        ORDER BY hop_count ASC, discovery_timestamp DESC
        "#,
        relationship_type
    )
    .fetch_all(pool)
    .await;

    match records {
        Ok(records) => Ok(records),
        Err(e) => Err(HackerdexError::DatabaseError(format!(
            "Failed to get trace links by relationship type: {}",
            e
        ))),
    }
}

/// Get addresses by multiple criteria (categories and risk levels)
pub async fn get_addresses_by_criteria(
    pool: &PgPool,
    categories: &[String],
    risk_levels: &[String],
) -> Result<Vec<AddressRecord>, HackerdexError> {
    // Build a dynamic query based on the provided criteria
    let mut query_builder = sqlx::QueryBuilder::new(
        "SELECT address, entity_name, category, risk_level, source_of_info, 
        confidence_score, notes, created_at, updated_at 
        FROM known_addresses WHERE 1=1 ",
    );

    // Add category filtering if categories are provided
    if !categories.is_empty() {
        query_builder.push(" AND category IN (");
        let mut separated = query_builder.separated(", ");
        for category in categories {
            separated.push_bind(category);
        }
        separated.push_unseparated(")");
    }

    // Add risk level filtering if risk levels are provided
    if !risk_levels.is_empty() {
        query_builder.push(" AND risk_level IN (");
        let mut separated = query_builder.separated(", ");
        for risk_level in risk_levels {
            separated.push_bind(risk_level);
        }
        separated.push_unseparated(")");
    }

    // Order by risk level in descending order (Critical, High, Medium, Low)
    query_builder.push(
        " ORDER BY 
        CASE 
            WHEN risk_level = 'Critical' THEN 1 
            WHEN risk_level = 'High' THEN 2 
            WHEN risk_level = 'Medium' THEN 3 
            WHEN risk_level = 'Low' THEN 4 
            ELSE 5 
        END ASC",
    );

    // Build and execute the query
    let query = query_builder.build_query_as::<AddressRecord>();
    match query.fetch_all(pool).await {
        Ok(records) => Ok(records),
        Err(e) => Err(HackerdexError::DatabaseError(format!(
            "Failed to get addresses by criteria: {}",
            e
        ))),
    }
}

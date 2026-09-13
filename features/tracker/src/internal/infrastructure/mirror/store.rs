use chrono::{DateTime, Utc};
use sqlx::{Row, SqlitePool};

use crate::internal::domain::{Issue, IssueFilter, IssueId, IssueRelation, ProjectRef};

use super::mapping::{
    MirrorIssueRow, issue_from_row, labels_to_json, parse_ts, relation_from_wire, relation_to_wire,
    status_to_wire,
};

/// Owns every SQL statement the mirror runs.
///
/// The rest of the crate only sees the domain-shaped API this
/// exposes; sqlx types never leak out.
#[derive(Clone)]
pub struct MirrorStore {
    pool: SqlitePool,
}

impl MirrorStore {
    #[must_use]
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// # Errors
    /// Any SQL failure (I/O, corruption, migration mismatch).
    pub async fn upsert_issue(
        &self,
        project: &ProjectRef,
        issue: &Issue,
        fetched_at: DateTime<Utc>,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            "INSERT INTO mirror_issues \
             (project, issue_id, title, status, labels_json, author, created_at, updated_at, fetched_at, description, milestone) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?) \
             ON CONFLICT (project, issue_id) DO UPDATE SET \
                title = excluded.title, \
                status = excluded.status, \
                labels_json = excluded.labels_json, \
                author = excluded.author, \
                created_at = excluded.created_at, \
                updated_at = excluded.updated_at, \
                fetched_at = excluded.fetched_at, \
                description = excluded.description, \
                milestone = excluded.milestone",
        )
        .bind(project.path())
        .bind(&issue.id.0)
        .bind(&issue.title)
        .bind(status_to_wire(issue.status))
        .bind(labels_to_json(&issue.labels))
        .bind(&issue.author)
        .bind(issue.created_at.to_rfc3339())
        .bind(issue.updated_at.to_rfc3339())
        .bind(fetched_at.to_rfc3339())
        .bind(issue.description.as_deref().unwrap_or(""))
        .bind(issue.milestone.as_deref().unwrap_or(""))
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// # Errors
    /// Any SQL failure. Runs in a transaction: relations are
    /// replaced atomically, never partially.
    pub async fn replace_relations(
        &self,
        project: &ProjectRef,
        id: &IssueId,
        relations: &[IssueRelation],
    ) -> Result<(), sqlx::Error> {
        let mut tx = self.pool.begin().await?;
        sqlx::query("DELETE FROM mirror_relations WHERE project = ? AND issue_id = ?")
            .bind(project.path())
            .bind(&id.0)
            .execute(&mut *tx)
            .await?;
        for rel in relations {
            let (kind, target) = relation_to_wire(rel);
            sqlx::query(
                "INSERT INTO mirror_relations (project, issue_id, kind, target_id) \
                 VALUES (?, ?, ?, ?)",
            )
            .bind(project.path())
            .bind(&id.0)
            .bind(kind)
            .bind(&target.0)
            .execute(&mut *tx)
            .await?;
        }
        tx.commit().await
    }

    /// # Errors
    /// Any SQL failure.
    pub async fn get_issue(
        &self,
        project: &ProjectRef,
        id: &IssueId,
    ) -> Result<Option<Issue>, sqlx::Error> {
        let row = sqlx::query(
            "SELECT issue_id, title, status, labels_json, author, created_at, updated_at, description, milestone \
             FROM mirror_issues WHERE project = ? AND issue_id = ?",
        )
        .bind(project.path())
        .bind(&id.0)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.and_then(|r| {
            issue_from_row(MirrorIssueRow {
                id: r.get("issue_id"),
                title: r.get("title"),
                status: r.get("status"),
                labels_json: r.get("labels_json"),
                author: r.get("author"),
                created_at: r.get("created_at"),
                updated_at: r.get("updated_at"),
                description: r.get("description"),
                milestone: r.get("milestone"),
            })
        }))
    }

    /// List mirrored issues for a project, applying the filter in
    /// SQL so we don't drag the full table across the wire.
    ///
    /// # Errors
    /// Any SQL failure.
    pub async fn list_issues(
        &self,
        project: &ProjectRef,
        filter: &IssueFilter,
    ) -> Result<Vec<Issue>, sqlx::Error> {
        let mut sql = String::from(
            "SELECT issue_id, title, status, labels_json, author, created_at, updated_at, description, milestone \
             FROM mirror_issues WHERE project = ?",
        );
        if filter.status.is_some() {
            sql.push_str(" AND status = ?");
        }
        if let Some(_ts) = filter.updated_after {
            sql.push_str(" AND updated_at >= ?");
        }
        // Filtered in SQL rather than after the fact: a plan-progress
        // question about one milestone must not drag the project's
        // whole issue list through memory to discard most of it.
        if filter.milestone.is_some() {
            sql.push_str(" AND milestone = ?");
        }
        sql.push_str(" ORDER BY updated_at DESC");

        let mut q = sqlx::query(&sql).bind(project.path());
        if let Some(status) = filter.status {
            q = q.bind(status_to_wire(status));
        }
        if let Some(ts) = filter.updated_after {
            q = q.bind(ts.to_rfc3339());
        }
        if let Some(milestone) = &filter.milestone {
            q = q.bind(milestone.clone());
        }
        let rows = q.fetch_all(&self.pool).await?;

        let mut out = Vec::with_capacity(rows.len());
        for r in rows {
            let issue = issue_from_row(MirrorIssueRow {
                id: r.get("issue_id"),
                title: r.get("title"),
                status: r.get("status"),
                labels_json: r.get("labels_json"),
                author: r.get("author"),
                created_at: r.get("created_at"),
                updated_at: r.get("updated_at"),
                description: r.get("description"),
                milestone: r.get("milestone"),
            });
            if let Some(issue) = issue
                && labels_cover(&issue, &filter.labels)
            {
                out.push(issue);
            }
        }
        Ok(out)
    }

    /// # Errors
    /// Any SQL failure.
    pub async fn list_relations(
        &self,
        project: &ProjectRef,
        id: &IssueId,
    ) -> Result<Vec<IssueRelation>, sqlx::Error> {
        let rows = sqlx::query(
            "SELECT kind, target_id FROM mirror_relations \
             WHERE project = ? AND issue_id = ?",
        )
        .bind(project.path())
        .bind(&id.0)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows
            .into_iter()
            .filter_map(|r| {
                let kind: String = r.get("kind");
                let target: String = r.get("target_id");
                relation_from_wire(&kind, target)
            })
            .collect())
    }

    /// # Errors
    /// Any SQL failure.
    pub async fn last_sync(
        &self,
        project: &ProjectRef,
    ) -> Result<Option<DateTime<Utc>>, sqlx::Error> {
        let row =
            sqlx::query("SELECT last_successful_sync FROM mirror_sync_cursor WHERE project = ?")
                .bind(project.path())
                .fetch_optional(&self.pool)
                .await?;
        Ok(row.and_then(|r| {
            let ts_str: String = r.get("last_successful_sync");
            parse_ts(&ts_str)
        }))
    }

    /// # Errors
    /// Any SQL failure.
    pub async fn set_last_sync(
        &self,
        project: &ProjectRef,
        ts: DateTime<Utc>,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            "INSERT INTO mirror_sync_cursor (project, last_successful_sync) VALUES (?, ?) \
             ON CONFLICT (project) DO UPDATE SET last_successful_sync = excluded.last_successful_sync",
        )
        .bind(project.path())
        .bind(ts.to_rfc3339())
        .execute(&self.pool)
        .await?;
        Ok(())
    }
}

fn labels_cover(issue: &Issue, wanted: &[crate::internal::domain::Label]) -> bool {
    wanted.iter().all(|w| issue.labels.iter().any(|l| l == w))
}

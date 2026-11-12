//! Store: writes one visit to D1. The adapters in lib.rs turn a request into a Hit; this only writes it.
use crate::s;
use crate::types::Hit;
use worker::*;

pub(crate) async fn record(db: &D1Database, h: &Hit) -> Result<()> {
    let hit = db
        .prepare(
            "INSERT INTO hits (day, path, kind, ref, referrer_host, audience, operator, product, n) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,1) \
             ON CONFLICT (day, path, kind, ref, referrer_host, audience, operator, product) DO UPDATE SET n = n + 1",
        )
        .bind(&[s(&h.day), s(&h.path), s(h.kind.name()), s(&h.ref_tag), s(&h.referrer_host), s(h.audience.name()), s(&h.operator), s(&h.product)])?;
    let mut batch = vec![hit];
    if !h.referrer_path.is_empty() {
        batch.push(
            db.prepare("INSERT INTO internal_hits (day, path, from_path, n) VALUES (?1,?2,?3,1) ON CONFLICT (day, path, from_path) DO UPDATE SET n = n + 1")
                .bind(&[s(&h.day), s(&h.path), s(&h.referrer_path)])?,
        );
    }
    // one D1 batch runs in one transaction: both counts are stored, or neither (#1727)
    db.batch(batch).await?;
    Ok(())
}

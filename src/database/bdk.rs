//! Persistence of the BDK wallet [`ChangeSet`] inside rgb-lib's own database.
//!
//! This reimplements, over sea-orm, the mapping that `bdk_chain` performs against `rusqlite`: it
//! must go through the very same [`DatabaseTransaction`] rgb-lib is already using (SQLite is
//! single-writer and rgb-lib holds the single pooled connection across BDK persist points, so a
//! separate connection would deadlock).
//!
//! The tables mirror `bdk_chain`'s schema one-to-one (names singularized to match rgb-lib's
//! convention, and versioning handled by rgb-lib's own migrator rather than a `bdk_schemas` table).
//! The signing and watch-only wallets of a fingerprint share the same store: their persisted
//! `ChangeSet` is identical (BDK keeps only the public descriptor and the private keys live in the
//! in-memory signers), so there is nothing to distinguish.

use super::*;

use crate::database::entities::{
    bdk_anchor, bdk_block, bdk_descriptor_derived_spk, bdk_descriptor_last_revealed, bdk_tx,
    bdk_txout, bdk_wallet as bdk_wallet_entity, bdk_wallet_locked_outpoint,
};

// The wallet-level data is stored in a single row, pinned to this ID, as in BDK.
const BDK_WALLET_ID: i32 = 0;

fn parse_err<E>(e: E) -> Error
where
    E: std::fmt::Display,
{
    Error::Internal {
        details: e.to_string(),
    }
}

fn encode_tx(tx: &BdkTransaction) -> Result<Vec<u8>, Error> {
    let mut bytes = Vec::new();
    tx.consensus_encode(&mut bytes)
        .map_err(std::io::Error::from)?;
    Ok(bytes)
}

fn decode_tx(bytes: &[u8]) -> Result<BdkTransaction, Error> {
    let tx = BdkTransaction::consensus_decode_from_finite_reader(&mut &bytes[..])
        .map_err(InternalError::from)?;
    Ok(tx)
}

impl DbTxn {
    /// Get the aggregate BDK [`ChangeSet`] from the database.
    pub(crate) fn get_bdk_changeset(&self) -> Result<ChangeSet, Error> {
        block_on(load_changeset(self.inner()))
    }

    /// Merge the given BDK [`ChangeSet`] delta into what is stored.
    pub(crate) fn update_bdk_changeset(&self, changeset: &ChangeSet) -> Result<(), Error> {
        block_on(persist_changeset(self.inner(), changeset))
    }
}

async fn load_changeset<C>(connection: &C) -> Result<ChangeSet, Error>
where
    C: ConnectionTrait,
{
    let mut changeset = ChangeSet::default();
    load_wallet(connection, &mut changeset).await?;
    load_locked_outpoints(connection, &mut changeset.locked_outpoints).await?;
    load_local_chain(connection, &mut changeset.local_chain).await?;
    load_tx_graph(connection, &mut changeset.tx_graph).await?;
    load_indexer(connection, &mut changeset.indexer).await?;
    Ok(changeset)
}

async fn persist_changeset<C>(connection: &C, changeset: &ChangeSet) -> Result<(), Error>
where
    C: ConnectionTrait,
{
    persist_wallet(connection, changeset).await?;
    persist_locked_outpoints(connection, &changeset.locked_outpoints).await?;
    persist_local_chain(connection, &changeset.local_chain).await?;
    persist_tx_graph(connection, &changeset.tx_graph).await?;
    persist_indexer(connection, &changeset.indexer).await?;
    Ok(())
}

async fn load_wallet<C>(connection: &C, changeset: &mut ChangeSet) -> Result<(), Error>
where
    C: ConnectionTrait,
{
    if let Some(row) = bdk_wallet_entity::Entity::find_by_id(BDK_WALLET_ID)
        .one(connection)
        .await?
    {
        changeset.descriptor = row
            .descriptor
            .map(|d| Descriptor::<DescriptorPublicKey>::from_str(&d))
            .transpose()
            .map_err(parse_err)?;
        changeset.change_descriptor = row
            .change_descriptor
            .map(|d| Descriptor::<DescriptorPublicKey>::from_str(&d))
            .transpose()
            .map_err(parse_err)?;
        changeset.network = row
            .network
            .map(|n| BdkNetwork::from_str(&n))
            .transpose()
            .map_err(parse_err)?;
    }
    Ok(())
}

async fn persist_wallet<C>(connection: &C, changeset: &ChangeSet) -> Result<(), Error>
where
    C: ConnectionTrait,
{
    if let Some(descriptor) = &changeset.descriptor {
        bdk_wallet_entity::Entity::insert(bdk_wallet_entity::ActiveModel {
            id: ActiveValue::Set(BDK_WALLET_ID),
            descriptor: ActiveValue::Set(Some(descriptor.to_string())),
            ..Default::default()
        })
        .on_conflict(
            OnConflict::column(bdk_wallet_entity::Column::Id)
                .update_column(bdk_wallet_entity::Column::Descriptor)
                .to_owned(),
        )
        .exec(connection)
        .await?;
    }
    if let Some(change_descriptor) = &changeset.change_descriptor {
        bdk_wallet_entity::Entity::insert(bdk_wallet_entity::ActiveModel {
            id: ActiveValue::Set(BDK_WALLET_ID),
            change_descriptor: ActiveValue::Set(Some(change_descriptor.to_string())),
            ..Default::default()
        })
        .on_conflict(
            OnConflict::column(bdk_wallet_entity::Column::Id)
                .update_column(bdk_wallet_entity::Column::ChangeDescriptor)
                .to_owned(),
        )
        .exec(connection)
        .await?;
    }
    if let Some(network) = &changeset.network {
        bdk_wallet_entity::Entity::insert(bdk_wallet_entity::ActiveModel {
            id: ActiveValue::Set(BDK_WALLET_ID),
            network: ActiveValue::Set(Some(network.to_string())),
            ..Default::default()
        })
        .on_conflict(
            OnConflict::column(bdk_wallet_entity::Column::Id)
                .update_column(bdk_wallet_entity::Column::Network)
                .to_owned(),
        )
        .exec(connection)
        .await?;
    }
    Ok(())
}

async fn load_locked_outpoints<C>(
    connection: &C,
    changeset: &mut locked_outpoints::ChangeSet,
) -> Result<(), Error>
where
    C: ConnectionTrait,
{
    for row in bdk_wallet_locked_outpoint::Entity::find()
        .all(connection)
        .await?
    {
        let outpoint = OutPoint::new(
            Txid::from_str(&row.txid).map_err(parse_err)?,
            row.vout as u32,
        );
        changeset.outpoints.insert(outpoint, true);
    }
    Ok(())
}

async fn persist_locked_outpoints<C>(
    connection: &C,
    changeset: &locked_outpoints::ChangeSet,
) -> Result<(), Error>
where
    C: ConnectionTrait,
{
    for (outpoint, is_locked) in &changeset.outpoints {
        if *is_locked {
            let res = bdk_wallet_locked_outpoint::Entity::insert(
                bdk_wallet_locked_outpoint::ActiveModel {
                    txid: ActiveValue::Set(outpoint.txid.to_string()),
                    vout: ActiveValue::Set(outpoint.vout as i64),
                },
            )
            .on_conflict(
                OnConflict::columns([
                    bdk_wallet_locked_outpoint::Column::Txid,
                    bdk_wallet_locked_outpoint::Column::Vout,
                ])
                .do_nothing()
                .to_owned(),
            )
            .exec(connection)
            .await;
            ignore_not_inserted(res)?;
        } else {
            bdk_wallet_locked_outpoint::Entity::delete_many()
                .filter(bdk_wallet_locked_outpoint::Column::Txid.eq(outpoint.txid.to_string()))
                .filter(bdk_wallet_locked_outpoint::Column::Vout.eq(outpoint.vout as i64))
                .exec(connection)
                .await?;
        }
    }
    Ok(())
}

async fn load_local_chain<C>(
    connection: &C,
    changeset: &mut local_chain::ChangeSet,
) -> Result<(), Error>
where
    C: ConnectionTrait,
{
    for row in bdk_block::Entity::find().all(connection).await? {
        let hash = BlockHash::from_str(&row.block_hash).map_err(parse_err)?;
        changeset.blocks.insert(row.block_height as u32, Some(hash));
    }
    Ok(())
}

async fn persist_local_chain<C>(
    connection: &C,
    changeset: &local_chain::ChangeSet,
) -> Result<(), Error>
where
    C: ConnectionTrait,
{
    for (height, hash) in &changeset.blocks {
        match hash {
            Some(hash) => {
                bdk_block::Entity::insert(bdk_block::ActiveModel {
                    block_height: ActiveValue::Set(*height as i64),
                    block_hash: ActiveValue::Set(hash.to_string()),
                })
                .on_conflict(
                    OnConflict::column(bdk_block::Column::BlockHeight)
                        .update_column(bdk_block::Column::BlockHash)
                        .to_owned(),
                )
                .exec(connection)
                .await?;
            }
            None => {
                bdk_block::Entity::delete_many()
                    .filter(bdk_block::Column::BlockHeight.eq(*height as i64))
                    .exec(connection)
                    .await?;
            }
        }
    }
    Ok(())
}

async fn load_tx_graph<C>(
    connection: &C,
    changeset: &mut tx_graph::ChangeSet<ConfirmationBlockTime>,
) -> Result<(), Error>
where
    C: ConnectionTrait,
{
    for row in bdk_tx::Entity::find().all(connection).await? {
        let txid = Txid::from_str(&row.txid).map_err(parse_err)?;
        if let Some(raw_tx) = row.raw_tx {
            changeset.txs.insert(Arc::new(decode_tx(&raw_tx)?));
        }
        if let Some(first_seen) = row.first_seen {
            changeset.first_seen.insert(txid, first_seen as u64);
        }
        if let Some(last_seen) = row.last_seen {
            changeset.last_seen.insert(txid, last_seen as u64);
        }
        if let Some(last_evicted) = row.last_evicted {
            changeset.last_evicted.insert(txid, last_evicted as u64);
        }
    }
    for row in bdk_txout::Entity::find().all(connection).await? {
        let outpoint = OutPoint::new(
            Txid::from_str(&row.txid).map_err(parse_err)?,
            row.vout as u32,
        );
        changeset.txouts.insert(
            outpoint,
            TxOut {
                value: BdkAmount::from_sat(row.value as u64),
                script_pubkey: ScriptBuf::from_bytes(row.script),
            },
        );
    }
    for row in bdk_anchor::Entity::find().all(connection).await? {
        let txid = Txid::from_str(&row.txid).map_err(parse_err)?;
        let hash = BlockHash::from_str(&row.block_hash).map_err(parse_err)?;
        changeset.anchors.insert((
            ConfirmationBlockTime {
                block_id: BlockId {
                    height: row.block_height as u32,
                    hash,
                },
                confirmation_time: row.confirmation_time as u64,
            },
            txid,
        ));
    }
    Ok(())
}

async fn persist_tx_graph<C>(
    connection: &C,
    changeset: &tx_graph::ChangeSet<ConfirmationBlockTime>,
) -> Result<(), Error>
where
    C: ConnectionTrait,
{
    for tx in &changeset.txs {
        bdk_tx::Entity::insert(bdk_tx::ActiveModel {
            txid: ActiveValue::Set(tx.compute_txid().to_string()),
            raw_tx: ActiveValue::Set(Some(encode_tx(tx)?)),
            ..Default::default()
        })
        .on_conflict(
            OnConflict::column(bdk_tx::Column::Txid)
                .update_column(bdk_tx::Column::RawTx)
                .to_owned(),
        )
        .exec(connection)
        .await?;
    }
    for (txid, first_seen) in &changeset.first_seen {
        upsert_tx_timestamp(
            connection,
            txid,
            bdk_tx::Column::FirstSeen,
            *first_seen,
            |m, v| m.first_seen = ActiveValue::Set(Some(v)),
        )
        .await?;
    }
    for (txid, last_seen) in &changeset.last_seen {
        upsert_tx_timestamp(
            connection,
            txid,
            bdk_tx::Column::LastSeen,
            *last_seen,
            |m, v| m.last_seen = ActiveValue::Set(Some(v)),
        )
        .await?;
    }
    for (txid, last_evicted) in &changeset.last_evicted {
        upsert_tx_timestamp(
            connection,
            txid,
            bdk_tx::Column::LastEvicted,
            *last_evicted,
            |m, v| m.last_evicted = ActiveValue::Set(Some(v)),
        )
        .await?;
    }
    for (outpoint, txout) in &changeset.txouts {
        bdk_txout::Entity::insert(bdk_txout::ActiveModel {
            txid: ActiveValue::Set(outpoint.txid.to_string()),
            vout: ActiveValue::Set(outpoint.vout as i64),
            value: ActiveValue::Set(txout.value.to_sat() as i64),
            script: ActiveValue::Set(txout.script_pubkey.to_bytes()),
        })
        .on_conflict(
            OnConflict::columns([bdk_txout::Column::Txid, bdk_txout::Column::Vout])
                .update_columns([bdk_txout::Column::Value, bdk_txout::Column::Script])
                .to_owned(),
        )
        .exec(connection)
        .await?;
    }
    for (anchor, txid) in &changeset.anchors {
        // the anchor references its transaction, which may not be stored in full yet; insert a
        // stub row so the foreign key holds, exactly as bdk_chain does
        let res = bdk_tx::Entity::insert(bdk_tx::ActiveModel {
            txid: ActiveValue::Set(txid.to_string()),
            ..Default::default()
        })
        .on_conflict(
            OnConflict::column(bdk_tx::Column::Txid)
                .do_nothing()
                .to_owned(),
        )
        .exec(connection)
        .await;
        ignore_not_inserted(res)?;
        bdk_anchor::Entity::insert(bdk_anchor::ActiveModel {
            txid: ActiveValue::Set(txid.to_string()),
            block_height: ActiveValue::Set(anchor.block_id.height as i64),
            block_hash: ActiveValue::Set(anchor.block_id.hash.to_string()),
            confirmation_time: ActiveValue::Set(anchor.confirmation_time as i64),
        })
        .on_conflict(
            OnConflict::columns([
                bdk_anchor::Column::Txid,
                bdk_anchor::Column::BlockHeight,
                bdk_anchor::Column::BlockHash,
            ])
            .update_column(bdk_anchor::Column::ConfirmationTime)
            .to_owned(),
        )
        .exec(connection)
        .await?;
    }
    Ok(())
}

async fn upsert_tx_timestamp<C, F>(
    connection: &C,
    txid: &Txid,
    column: bdk_tx::Column,
    value: u64,
    set_field: F,
) -> Result<(), Error>
where
    C: ConnectionTrait,
    F: FnOnce(&mut bdk_tx::ActiveModel, i64),
{
    let mut model = bdk_tx::ActiveModel {
        txid: ActiveValue::Set(txid.to_string()),
        ..Default::default()
    };
    set_field(&mut model, value as i64);
    bdk_tx::Entity::insert(model)
        .on_conflict(
            OnConflict::column(bdk_tx::Column::Txid)
                .update_column(column)
                .to_owned(),
        )
        .exec(connection)
        .await?;
    Ok(())
}

async fn load_indexer<C>(
    connection: &C,
    changeset: &mut keychain_txout::ChangeSet,
) -> Result<(), Error>
where
    C: ConnectionTrait,
{
    for row in bdk_descriptor_last_revealed::Entity::find()
        .all(connection)
        .await?
    {
        let descriptor_id = DescriptorId::from_str(&row.descriptor_id).map_err(parse_err)?;
        changeset
            .last_revealed
            .insert(descriptor_id, row.last_revealed as u32);
    }
    for row in bdk_descriptor_derived_spk::Entity::find()
        .all(connection)
        .await?
    {
        let descriptor_id = DescriptorId::from_str(&row.descriptor_id).map_err(parse_err)?;
        changeset
            .spk_cache
            .entry(descriptor_id)
            .or_default()
            .insert(row.spk_index as u32, ScriptBuf::from_bytes(row.spk));
    }
    Ok(())
}

async fn persist_indexer<C>(
    connection: &C,
    changeset: &keychain_txout::ChangeSet,
) -> Result<(), Error>
where
    C: ConnectionTrait,
{
    for (descriptor_id, last_revealed) in &changeset.last_revealed {
        bdk_descriptor_last_revealed::Entity::insert(bdk_descriptor_last_revealed::ActiveModel {
            descriptor_id: ActiveValue::Set(descriptor_id.to_string()),
            last_revealed: ActiveValue::Set(*last_revealed as i64),
        })
        .on_conflict(
            OnConflict::column(bdk_descriptor_last_revealed::Column::DescriptorId)
                .update_column(bdk_descriptor_last_revealed::Column::LastRevealed)
                .to_owned(),
        )
        .exec(connection)
        .await?;
    }
    for (descriptor_id, spks) in &changeset.spk_cache {
        for (spk_index, spk) in spks {
            bdk_descriptor_derived_spk::Entity::insert(bdk_descriptor_derived_spk::ActiveModel {
                descriptor_id: ActiveValue::Set(descriptor_id.to_string()),
                spk_index: ActiveValue::Set(*spk_index as i64),
                spk: ActiveValue::Set(spk.to_bytes()),
            })
            .on_conflict(
                OnConflict::columns([
                    bdk_descriptor_derived_spk::Column::DescriptorId,
                    bdk_descriptor_derived_spk::Column::SpkIndex,
                ])
                .update_column(bdk_descriptor_derived_spk::Column::Spk)
                .to_owned(),
            )
            .exec(connection)
            .await?;
        }
    }
    Ok(())
}

/// A `do_nothing` upsert reports [`DbErr::RecordNotInserted`] when the row already exists; that is
/// the intended outcome for insert-or-ignore, so treat it as success.
fn ignore_not_inserted<T>(res: Result<T, DbErr>) -> Result<(), Error> {
    match res {
        Ok(_) | Err(DbErr::RecordNotInserted) => Ok(()),
        Err(e) => Err(e.into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use bdk_wallet::bitcoin::{TxIn, absolute::LockTime, transaction::Version};

    use crate::wallet::core::RGB_LIB_DB_NAME;

    const TPUB: &str = "tpubD6NzVbkrYhZ4WLczPJWReQycCJdd6YVWXubbVUFnJ5KgU5MDQrD998ZJLSmaB7GVcCnJSDWprxmrGkJ6SvgQC6QAffVpqSvonXmeizXcrkN";

    fn test_db() -> (RgbLibDatabase, TempDir) {
        let dir = TempDir::new().unwrap();
        let db_path = dir.path().join(RGB_LIB_DB_NAME);
        let connection_string = format!("sqlite:{}?mode=rwc", db_path.display());
        let connection = block_on(Database::connect(connection_string)).unwrap();
        block_on(Migrator::up(&connection, None)).unwrap();
        (RgbLibDatabase::new(connection), dir)
    }

    // Persist a fully-populated ChangeSet and read it back, exercising every table (wallet-level,
    // locked outpoints, blocks, tx_graph, anchors, txouts, indexer) which the offline wallet tests
    // do not reach.
    #[test]
    fn changeset_round_trip() {
        let (db, _dir) = test_db();

        let tx = BdkTransaction {
            version: Version::TWO,
            lock_time: LockTime::ZERO,
            input: vec![TxIn::default()],
            output: vec![TxOut {
                value: BdkAmount::from_sat(1000),
                script_pubkey: ScriptBuf::from_bytes(vec![0x51]),
            }],
        };
        let txid = tx.compute_txid();
        let descriptor_id = DescriptorId::from_str(&"03".repeat(32)).unwrap();
        let descriptor =
            Descriptor::<DescriptorPublicKey>::from_str(&format!("wpkh({TPUB}/0/*)")).unwrap();
        let change_descriptor =
            Descriptor::<DescriptorPublicKey>::from_str(&format!("wpkh({TPUB}/1/*)")).unwrap();

        let mut changeset = ChangeSet {
            descriptor: Some(descriptor),
            change_descriptor: Some(change_descriptor),
            network: Some(BdkNetwork::Regtest),
            ..Default::default()
        };
        changeset
            .local_chain
            .blocks
            .insert(100, Some(BlockHash::from_byte_array([7; 32])));
        changeset.tx_graph.txs.insert(Arc::new(tx.clone()));
        changeset.tx_graph.txouts.insert(
            OutPoint::new(txid, 0),
            TxOut {
                value: BdkAmount::from_sat(2000),
                script_pubkey: ScriptBuf::from_bytes(vec![0x52]),
            },
        );
        changeset.tx_graph.anchors.insert((
            ConfirmationBlockTime {
                block_id: BlockId {
                    height: 123,
                    hash: BlockHash::from_byte_array([9; 32]),
                },
                confirmation_time: 456,
            },
            txid,
        ));
        changeset.tx_graph.first_seen.insert(txid, 111);
        changeset.tx_graph.last_seen.insert(txid, 222);
        changeset.tx_graph.last_evicted.insert(txid, 0);
        changeset.indexer.last_revealed.insert(descriptor_id, 5);
        changeset
            .indexer
            .spk_cache
            .entry(descriptor_id)
            .or_default()
            .insert(7, ScriptBuf::from_bytes(vec![0x53]));
        changeset
            .locked_outpoints
            .outpoints
            .insert(OutPoint::new(txid, 1), true);

        // persist twice through separate transactions to also cover the merge/upsert path
        for _ in 0..2 {
            let txn = db.begin_transaction().unwrap();
            txn.update_bdk_changeset(&changeset).unwrap();
            txn.commit().unwrap();
        }

        let txn = db.begin_transaction().unwrap();
        let loaded = txn.get_bdk_changeset().unwrap();
        txn.commit().unwrap();

        assert_eq!(loaded, changeset);
    }
}

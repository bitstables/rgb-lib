use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // wallet-level data (descriptors and network); a single row pinned to ID 0, as in BDK
        manager
            .create_table(
                Table::create()
                    .table(BdkWallet::Table)
                    .if_not_exists()
                    .col(
                        integer(BdkWallet::Id)
                            .primary_key()
                            .check(Expr::col(BdkWallet::Id).eq(0)),
                    )
                    .col(string_null(BdkWallet::Descriptor))
                    .col(string_null(BdkWallet::ChangeDescriptor))
                    .col(string_null(BdkWallet::Network))
                    .to_owned(),
            )
            .await?;

        // locked outpoints
        manager
            .create_table(
                Table::create()
                    .table(BdkWalletLockedOutpoint::Table)
                    .if_not_exists()
                    .col(string(BdkWalletLockedOutpoint::Txid))
                    .col(big_integer(BdkWalletLockedOutpoint::Vout))
                    .primary_key(
                        Index::create()
                            .col(BdkWalletLockedOutpoint::Txid)
                            .col(BdkWalletLockedOutpoint::Vout),
                    )
                    .to_owned(),
            )
            .await?;

        // local chain blocks
        manager
            .create_table(
                Table::create()
                    .table(BdkBlock::Table)
                    .if_not_exists()
                    .col(big_integer(BdkBlock::BlockHeight).primary_key())
                    .col(string(BdkBlock::BlockHash))
                    .to_owned(),
            )
            .await?;

        // full transactions and their seen/evicted timestamps
        manager
            .create_table(
                Table::create()
                    .table(BdkTx::Table)
                    .if_not_exists()
                    .col(string(BdkTx::Txid).primary_key())
                    .col(binary_null(BdkTx::RawTx))
                    .col(big_integer_null(BdkTx::FirstSeen))
                    .col(big_integer_null(BdkTx::LastSeen))
                    .col(big_integer_null(BdkTx::LastEvicted))
                    .to_owned(),
            )
            .await?;

        // floating txouts
        manager
            .create_table(
                Table::create()
                    .table(BdkTxout::Table)
                    .if_not_exists()
                    .col(string(BdkTxout::Txid))
                    .col(big_integer(BdkTxout::Vout))
                    .col(big_integer(BdkTxout::Value))
                    .col(binary(BdkTxout::Script))
                    .primary_key(Index::create().col(BdkTxout::Txid).col(BdkTxout::Vout))
                    .to_owned(),
            )
            .await?;

        // anchors (transaction confirmations); references its transaction like BDK's own schema
        manager
            .create_table(
                Table::create()
                    .table(BdkAnchor::Table)
                    .if_not_exists()
                    .col(string(BdkAnchor::Txid))
                    .col(big_integer(BdkAnchor::BlockHeight))
                    .col(string(BdkAnchor::BlockHash))
                    .col(big_integer(BdkAnchor::ConfirmationTime))
                    .primary_key(
                        Index::create()
                            .col(BdkAnchor::Txid)
                            .col(BdkAnchor::BlockHeight)
                            .col(BdkAnchor::BlockHash),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk-bdkanchor-bdktx")
                            .from(BdkAnchor::Table, BdkAnchor::Txid)
                            .to(BdkTx::Table, BdkTx::Txid)
                            .on_delete(ForeignKeyAction::Cascade)
                            .on_update(ForeignKeyAction::Restrict),
                    )
                    .to_owned(),
            )
            .await?;

        // last revealed index per descriptor
        manager
            .create_table(
                Table::create()
                    .table(BdkDescriptorLastRevealed::Table)
                    .if_not_exists()
                    .col(string(BdkDescriptorLastRevealed::DescriptorId).primary_key())
                    .col(big_integer(BdkDescriptorLastRevealed::LastRevealed))
                    .to_owned(),
            )
            .await?;

        // derived SPK cache
        manager
            .create_table(
                Table::create()
                    .table(BdkDescriptorDerivedSpk::Table)
                    .if_not_exists()
                    .col(string(BdkDescriptorDerivedSpk::DescriptorId))
                    .col(big_integer(BdkDescriptorDerivedSpk::SpkIndex))
                    .col(binary(BdkDescriptorDerivedSpk::Spk))
                    .primary_key(
                        Index::create()
                            .col(BdkDescriptorDerivedSpk::DescriptorId)
                            .col(BdkDescriptorDerivedSpk::SpkIndex),
                    )
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(
                Table::drop()
                    .table(BdkDescriptorDerivedSpk::Table)
                    .to_owned(),
            )
            .await?;
        manager
            .drop_table(
                Table::drop()
                    .table(BdkDescriptorLastRevealed::Table)
                    .to_owned(),
            )
            .await?;
        manager
            .drop_table(Table::drop().table(BdkAnchor::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(BdkTxout::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(BdkTx::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(BdkBlock::Table).to_owned())
            .await?;
        manager
            .drop_table(
                Table::drop()
                    .table(BdkWalletLockedOutpoint::Table)
                    .to_owned(),
            )
            .await?;
        manager
            .drop_table(Table::drop().table(BdkWallet::Table).to_owned())
            .await?;

        Ok(())
    }
}

#[derive(DeriveIden)]
enum BdkWallet {
    Table,
    Id,
    Descriptor,
    ChangeDescriptor,
    Network,
}

#[derive(DeriveIden)]
enum BdkWalletLockedOutpoint {
    Table,
    Txid,
    Vout,
}

#[derive(DeriveIden)]
enum BdkBlock {
    Table,
    BlockHeight,
    BlockHash,
}

#[derive(DeriveIden)]
enum BdkTx {
    Table,
    Txid,
    RawTx,
    FirstSeen,
    LastSeen,
    LastEvicted,
}

#[derive(DeriveIden)]
enum BdkTxout {
    Table,
    Txid,
    Vout,
    Value,
    Script,
}

#[derive(DeriveIden)]
enum BdkAnchor {
    Table,
    Txid,
    BlockHeight,
    BlockHash,
    ConfirmationTime,
}

#[derive(DeriveIden)]
enum BdkDescriptorLastRevealed {
    Table,
    DescriptorId,
    LastRevealed,
}

#[derive(DeriveIden)]
enum BdkDescriptorDerivedSpk {
    Table,
    DescriptorId,
    SpkIndex,
    Spk,
}

//! `SeaORM` Entity for BDK anchors (transaction confirmations).

use sea_orm::entity::prelude::*;

#[derive(Copy, Clone, Default, Debug, DeriveEntity)]
pub struct Entity;

impl EntityName for Entity {
    fn table_name(&self) -> &'static str {
        "bdk_anchor"
    }
}

#[derive(Clone, Debug, PartialEq, DeriveModel, DeriveActiveModel, Eq)]
pub struct Model {
    pub txid: String,
    pub block_height: i64,
    pub block_hash: String,
    pub confirmation_time: i64,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveColumn)]
pub enum Column {
    Txid,
    BlockHeight,
    BlockHash,
    ConfirmationTime,
}

#[derive(Copy, Clone, Debug, EnumIter, DerivePrimaryKey)]
pub enum PrimaryKey {
    Txid,
    BlockHeight,
    BlockHash,
}

impl PrimaryKeyTrait for PrimaryKey {
    type ValueType = (String, i64, String);
    fn auto_increment() -> bool {
        false
    }
}

#[derive(Copy, Clone, Debug, EnumIter)]
pub enum Relation {}

impl ColumnTrait for Column {
    type EntityName = Entity;
    fn def(&self) -> ColumnDef {
        match self {
            Self::Txid => ColumnType::String(StringLen::None).def(),
            Self::BlockHeight => ColumnType::BigInteger.def(),
            Self::BlockHash => ColumnType::String(StringLen::None).def(),
            Self::ConfirmationTime => ColumnType::BigInteger.def(),
        }
    }
}

impl RelationTrait for Relation {
    fn def(&self) -> RelationDef {
        panic!("No RelationDef")
    }
}

impl ActiveModelBehavior for ActiveModel {}

//! `SeaORM` Entity for BDK transactions and their seen/evicted timestamps.

use sea_orm::entity::prelude::*;

#[derive(Copy, Clone, Default, Debug, DeriveEntity)]
pub struct Entity;

impl EntityName for Entity {
    fn table_name(&self) -> &'static str {
        "bdk_tx"
    }
}

#[derive(Clone, Debug, PartialEq, DeriveModel, DeriveActiveModel, Eq)]
pub struct Model {
    pub txid: String,
    pub raw_tx: Option<Vec<u8>>,
    pub first_seen: Option<i64>,
    pub last_seen: Option<i64>,
    pub last_evicted: Option<i64>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveColumn)]
pub enum Column {
    Txid,
    RawTx,
    FirstSeen,
    LastSeen,
    LastEvicted,
}

#[derive(Copy, Clone, Debug, EnumIter, DerivePrimaryKey)]
pub enum PrimaryKey {
    Txid,
}

impl PrimaryKeyTrait for PrimaryKey {
    type ValueType = String;
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
            Self::RawTx => ColumnType::Blob.def().null(),
            Self::FirstSeen => ColumnType::BigInteger.def().null(),
            Self::LastSeen => ColumnType::BigInteger.def().null(),
            Self::LastEvicted => ColumnType::BigInteger.def().null(),
        }
    }
}

impl RelationTrait for Relation {
    fn def(&self) -> RelationDef {
        panic!("No RelationDef")
    }
}

impl ActiveModelBehavior for ActiveModel {}
